-- Sessions we can revoke at once (docs/auth/sessions-and-tokens.md, #13201). A session is a row;
-- every token we hand out is an opaque string whose SHA-256 is the row's key, so revoking is
-- deleting the row and takes effect on the next request. No JWTs.
--
--   web   __Host-oa_session: 30 days idle, 180 days absolute, a new token every 24 hours of use
--         (the old one still works for 60 seconds, for parallel tabs)
--   app   oa_at_ access token (1 hour) and oa_rt_ refresh token (90 days idle); each refresh is
--         spent once, and a spent one presented again ends its whole family
-- When those clocks run out is decided in auth-policy.coax; these rows only hold the times.
CREATE TABLE auth.sessions (
  token_sha256 text PRIMARY KEY CHECK (token_sha256 ~ '^[0-9a-f]{64}$'),
  id text NOT NULL UNIQUE,               -- ses_ + 16 hex: what the devices list shows and removes
  account text NOT NULL REFERENCES auth.accounts (id),
  via_kind text NOT NULL CHECK (via_kind IN ('email', 'nostr', 'passkey', 'device', 'recovery')),
  via_id bigint,                         -- the authenticator's row (auth.emails.id, auth.nostr_keys.id, ...)
  device bigint REFERENCES auth.devices (id),
  client text NOT NULL CHECK (client IN ('web', 'app')),
  level text NOT NULL CHECK (level IN ('L1', 'L2')),
  l1_at timestamptz,
  l2_at timestamptz,
  epoch int NOT NULL,                    -- the account's session_epoch when it was made
  name text,                             -- what the devices list calls it (the browser or app)
  country text,                          -- coarse location, kept 30 days
  created_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz NOT NULL DEFAULT now(),
  rotated_at timestamptz NOT NULL DEFAULT now(),
  prev_sha256 text,                      -- the token before the last rotation, good until prev_until
  prev_until timestamptz
);
CREATE INDEX sessions_account ON auth.sessions (account);
CREATE INDEX sessions_prev ON auth.sessions (prev_sha256) WHERE prev_sha256 IS NOT NULL;
CREATE TABLE auth.refresh (
  token_sha256 text PRIMARY KEY CHECK (token_sha256 ~ '^[0-9a-f]{64}$'),
  family text NOT NULL,                  -- every refresh token of one login
  session text NOT NULL REFERENCES auth.sessions (id) ON DELETE CASCADE,
  access_sha256 text NOT NULL UNIQUE,    -- the access token minted with it
  access_until timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  spent_at timestamptz
);
CREATE INDEX refresh_family ON auth.refresh (family);
-- "This wasn't me": a link in a notice that freezes the account. Only its SHA-256 is kept.
CREATE TABLE auth.freeze_links (
  token_sha256 text PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  used_at timestamptz
);

-- A new session for acct, by the authenticator (kind, id), at level. Its id.
CREATE FUNCTION auth.session_new(sha text, acct text, kind_ text, via bigint, level_ text, client_ text, name_ text, country_ text)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE sid text := 'ses_' || left(projects.sha(projects.token()), 16);
BEGIN
  INSERT INTO auth.sessions (token_sha256, id, account, via_kind, via_id, client, level, l1_at, l2_at, epoch, name, country)
  SELECT sha, sid, a.id, kind_, via, client_, level_, now(), CASE WHEN level_ = 'L2' THEN now() END, a.session_epoch,
         nullif(left(name_, 80), ''), nullif(left(country_, 2), '')
    FROM auth.accounts a WHERE a.id = acct AND a.deleted_at IS NULL AND a.frozen_at IS NULL;
  IF NOT FOUND THEN RETURN NULL; END IF;
  RETURN sid;
END $$;

-- What a session token speaks for: one row, or none. The times go to auth-policy.coax, which says
-- whether it is still good and whether it is due for a new token. The account's key (pubkey) is
-- the one it logged in with, else its newest, else NULL (an email-only account).
CREATE FUNCTION auth.session_look(sha text) RETURNS TABLE (
  id text, account text, kind text, verse_account text, level text, via_kind text, client text,
  created_ms bigint, last_seen_ms bigint, rotated_ms bigint, l2_ms bigint, now_ms bigint,
  current boolean, good_epoch boolean, frozen boolean, team boolean, pubkey text)
LANGUAGE sql STABLE AS $$
  SELECT s.id, s.account, a.kind, a.verse_account, s.level, s.via_kind, s.client,
         (extract(epoch FROM s.created_at) * 1000)::bigint, (extract(epoch FROM s.last_seen_at) * 1000)::bigint,
         (extract(epoch FROM s.rotated_at) * 1000)::bigint, (extract(epoch FROM s.l2_at) * 1000)::bigint,
         (extract(epoch FROM now()) * 1000)::bigint,
         s.token_sha256 = sha, s.epoch = a.session_epoch, a.frozen_at IS NOT NULL, a.team,
         coalesce((SELECT k.pubkey FROM auth.nostr_keys k WHERE s.via_kind = 'nostr' AND k.id = s.via_id),
                  (SELECT k.pubkey FROM auth.nostr_keys k WHERE k.account = a.id ORDER BY k.last_used_at DESC NULLS LAST, k.id DESC LIMIT 1))
    FROM auth.sessions s JOIN auth.accounts a ON a.id = s.account
   WHERE (s.token_sha256 = sha OR (s.prev_sha256 = sha AND s.prev_until > now())) AND a.deleted_at IS NULL
$$;
-- Seen now (at most once a minute, so a page's every request isn't a write).
CREATE FUNCTION auth.session_seen(sid text) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.sessions SET last_seen_at = now() WHERE id = sid AND last_seen_at < now() - interval '1 minute'
$$;
-- A new token for a session; the old one stays good for 60 seconds.
CREATE FUNCTION auth.session_rotate(sid text, old_sha text, new_sha text) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.sessions SET prev_sha256 = old_sha, prev_until = now() + interval '60 seconds',
                                      token_sha256 = new_sha, rotated_at = now()
              WHERE id = sid AND token_sha256 = old_sha RETURNING 1)
  SELECT EXISTS (SELECT 1 FROM u)
$$;
-- Log out: this session only.
CREATE FUNCTION auth.session_end(sid text) RETURNS boolean LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.sessions WHERE id = sid RETURNING 1) SELECT EXISTS (SELECT 1 FROM d)
$$;
-- Log out everywhere: every session and refresh family of the account, and the epoch moves on, so a
-- session made by a request already in flight is refused too. How many ended.
CREATE FUNCTION auth.sessions_end_all(acct text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.accounts SET session_epoch = session_epoch + 1 WHERE id = acct;
  DELETE FROM auth.sessions WHERE account = acct;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;
-- The devices list: each session with its name, client, how it logged in, when, and where.
CREATE FUNCTION auth.sessions_of(acct text, this_sid text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object(
           'id', s.id, 'name', coalesce(s.name, CASE s.client WHEN 'app' THEN 'App' ELSE 'Browser' END),
           'client', s.client, 'via', s.via_kind, 'level', s.level, 'country', s.country,
           'created_ms', (extract(epoch FROM s.created_at) * 1000)::bigint,
           'last_seen_ms', (extract(epoch FROM s.last_seen_at) * 1000)::bigint,
           'this', s.id = this_sid) ORDER BY s.last_seen_at DESC), '[]'::jsonb)
    FROM auth.sessions s WHERE s.account = acct
$$;
-- Freeze ("This wasn't me"): every session ends and the account takes no new session until it is
-- unfrozen by logging in at L2 (#13214).
CREATE FUNCTION auth.freeze(acct text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.accounts SET frozen_at = coalesce(frozen_at, now()) WHERE id = acct;
  n := auth.sessions_end_all(acct);
  RETURN n;
END $$;
-- A "This wasn't me" link for a notice: 14 days, once. The caller made the token and keeps it.
CREATE FUNCTION auth.freeze_link(acct text, sha text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.freeze_links (token_sha256, account, expires_at) VALUES (sha, acct, now() + interval '14 days')
$$;
-- Spends a freeze link: the account it froze, or NULL.
CREATE FUNCTION auth.freeze_by_link(sha text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE acct text;
BEGIN
  UPDATE auth.freeze_links SET used_at = now() WHERE token_sha256 = sha AND used_at IS NULL AND expires_at > now() RETURNING account INTO acct;
  IF acct IS NOT NULL THEN PERFORM auth.freeze(acct); END IF;
  RETURN acct;
END $$;

-- App tokens. A login's first pair: the refresh token starts a family.
CREATE FUNCTION auth.app_pair(sid text, family_ text, rt_sha text, at_sha text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.refresh (token_sha256, family, session, access_sha256, access_until)
  VALUES (rt_sha, family_, sid, at_sha, now() + interval '1 hour')
$$;
-- An access token: its session's token_sha256 (to look up as a session), or NULL when expired.
CREATE FUNCTION auth.access_session(at_sha text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.token_sha256 FROM auth.refresh r JOIN auth.sessions s ON s.id = r.session
   WHERE r.access_sha256 = at_sha AND r.access_until > now()
$$;
-- A refresh: spends rt and records the next pair. 'ok', 'reused' (the family is ended: someone
-- copied a token), 'idle' (90 days unused) or 'unknown'.
CREATE FUNCTION auth.app_refresh(rt_sha text, new_rt text, new_at text, OUT state text, OUT account text, OUT family text) LANGUAGE plpgsql AS $$
DECLARE r auth.refresh;
BEGIN
  SELECT * INTO r FROM auth.refresh WHERE token_sha256 = rt_sha FOR UPDATE;
  IF r.token_sha256 IS NULL THEN state := 'unknown'; RETURN; END IF;
  SELECT s.account INTO account FROM auth.sessions s WHERE s.id = r.session;
  family := r.family;
  IF r.spent_at IS NOT NULL THEN
    DELETE FROM auth.sessions s WHERE s.id IN (SELECT x.session FROM auth.refresh x WHERE x.family = r.family);
    state := 'reused'; RETURN;
  END IF;
  IF r.created_at < now() - interval '90 days' THEN
    DELETE FROM auth.sessions s WHERE s.id = r.session;
    state := 'idle'; RETURN;
  END IF;
  UPDATE auth.refresh SET spent_at = now() WHERE token_sha256 = rt_sha;
  INSERT INTO auth.refresh (token_sha256, family, session, access_sha256, access_until)
  VALUES (new_rt, r.family, r.session, new_at, now() + interval '1 hour');
  UPDATE auth.sessions SET last_seen_at = now() WHERE id = r.session;
  state := 'ok';
END $$;
