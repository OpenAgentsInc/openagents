-- A command line logs in from a browser (docs/auth/sessions-and-tokens.md, "A command line logs in
-- from a browser"; INVARIANTS.md "A command line's login from a browser is the browser's email
-- login, no more"). A browser logged in by email makes a one-time code (POST /api/auth/cli/code);
-- the command line spends it once, within 5 minutes, for an app's login of its own (POST
-- /api/auth/cli/redeem). The row keeps the code's SHA-256 only, and the browser session it came
-- from: the command line's login is that session's email, at L1, with that session's email proof
-- time, never a fresh one.
ALTER TABLE auth.pending DROP CONSTRAINT pending_kind_check;
ALTER TABLE auth.pending ADD CONSTRAINT pending_kind_check CHECK (kind IN ('email', 'nostr', 'phone', 'passkey', 'cli'));
CREATE INDEX pending_cli_code ON auth.pending (code_sha256) WHERE kind = 'cli';

-- A new code for the browser session sid: 'ok', or 'none' when sid is not a web session from an
-- email login. The same session's codes not yet spent end.
CREATE FUNCTION auth.cli_start(id_ text, code_sha text, sid text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE s auth.sessions;
BEGIN
  SELECT * INTO s FROM auth.sessions x WHERE x.id = sid;
  IF s.id IS NULL OR s.via_kind <> 'email' OR s.client <> 'web' THEN RETURN 'none'; END IF;
  DELETE FROM auth.pending WHERE kind = 'cli' AND session = sid AND done_at IS NULL;
  DELETE FROM auth.pending WHERE expires_at < now() - interval '1 day';
  INSERT INTO auth.pending (id, kind, code_sha256, account, session, client, expires_at)
  VALUES (id_, 'cli', code_sha, s.account, sid, 'app', now() + interval '5 minutes');
  RETURN 'ok';
END $$;

-- Spends a code, once: 'ok' with the account, the email row the browser logged in by and when;
-- 'none' (no such code, spent, or past its 5 minutes); 'ended' (the browser session that made it
-- has ended since, by logging out, a removal or logging out everywhere). Spent either way.
CREATE FUNCTION auth.cli_take(code_sha text, OUT state text, OUT account text, OUT via_id bigint, OUT l1_at timestamptz)
LANGUAGE plpgsql AS $$
DECLARE p auth.pending; s auth.sessions;
BEGIN
  SELECT * INTO p FROM auth.pending x WHERE x.kind = 'cli' AND x.code_sha256 = code_sha FOR UPDATE;
  IF p.id IS NULL OR p.done_at IS NOT NULL OR p.expires_at <= now() THEN state := 'none'; RETURN; END IF;
  UPDATE auth.pending SET done_at = now() WHERE id = p.id;
  SELECT * INTO s FROM auth.sessions x WHERE x.id = p.session;
  IF s.id IS NULL OR s.account <> p.account OR s.via_kind <> 'email' THEN state := 'ended'; RETURN; END IF;
  state := 'ok'; account := s.account; via_id := s.via_id; l1_at := s.l1_at;
END $$;

-- The command line's new session carries the browser's email proof time, not its own: a code is
-- not a fresh email code (auth-life-policy.coax al_proof reads it).
CREATE FUNCTION auth.cli_carry(sid text, l1 timestamptz) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.sessions SET l1_at = l1 WHERE id = sid AND level = 'L1'
$$;
