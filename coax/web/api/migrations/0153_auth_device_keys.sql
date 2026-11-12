-- Device keys that bind an app's refresh tokens to its hardware (docs/auth/sessions-and-tokens.md
-- "Native: refresh tokens bound to a device key", #13216). An app makes a P-256 key that can't be
-- exported (Secure Enclave, Android Keystore) and sends its public half with its login. The
-- session is bound to that key for good, and every refresh of it must carry OA-Device-Proof, the
-- key's signature over the method, the URL, a time and the refresh token's SHA-256. What is
-- allowed is decided in auth-device-policy.coax (#authority); these rows hold the facts, and the
-- functions below refuse on their own what the policy must never allow.
--
-- A device key binds its own login's refresh tokens and nothing else: it is not a way to log in,
-- and it doesn't raise a session's level. So it no longer counts as a stronger way in (it did in
-- 0148 and 0149, before any device existed): an email-only person who logs in on a phone keeps a
-- full session on the web. That changes when a device key can prove a step-up (#13217).

-- The same install may log in to one account after another, and a removed key's install may log
-- in again: a key is one live row per account, and removed rows stay as history.
ALTER TABLE auth.devices DROP CONSTRAINT devices_public_key_key;
CREATE UNIQUE INDEX devices_live_key ON auth.devices (account, public_key) WHERE revoked_at IS NULL;
CREATE INDEX devices_account ON auth.devices (account);
ALTER TABLE auth.devices ADD CONSTRAINT devices_key_shape CHECK (length(public_key) = 65 AND get_byte(public_key, 0) = 4);
-- Where the app says the key lives. We ask for no attestation, so this is the app's word, shown as such.
ALTER TABLE auth.devices ADD COLUMN store text NOT NULL DEFAULT 'software' CHECK (store IN ('enclave', 'strongbox', 'tee', 'software'));
CREATE INDEX sessions_device ON auth.sessions (device) WHERE device IS NOT NULL;

-- A session's device is set once and never changed or cleared: a bound login can't be unbound.
CREATE FUNCTION auth.session_device_fixed() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF OLD.device IS NOT NULL AND NEW.device IS DISTINCT FROM OLD.device THEN
    RAISE EXCEPTION 'auth.sessions: a session''s device key never changes' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER session_device_fixed BEFORE UPDATE OF device ON auth.sessions FOR EACH ROW EXECUTE FUNCTION auth.session_device_fixed();
-- A device belongs to one account. When a merge moves the row, its logins have ended: it is history.
CREATE FUNCTION auth.device_moved() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.account IS DISTINCT FROM OLD.account THEN NEW.revoked_at := coalesce(NEW.revoked_at, now()); END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER device_moved BEFORE UPDATE OF account ON auth.devices FOR EACH ROW EXECUTE FUNCTION auth.device_moved();

-- Proofs already used, by the SHA-256 of what was signed, kept ten minutes past their window.
CREATE TABLE auth.device_proofs (
  message_sha256 text PRIMARY KEY CHECK (message_sha256 ~ '^[0-9a-f]{64}$'),
  expires_at timestamptz NOT NULL
);
CREATE INDEX device_proofs_expires ON auth.device_proofs (expires_at);
-- True the first time a proof is seen; false ever after. Old rows go as new ones come.
CREATE FUNCTION auth.device_proof_once(sha text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  DELETE FROM auth.device_proofs WHERE expires_at < now();
  INSERT INTO auth.device_proofs (message_sha256, expires_at) VALUES (sha, now() + interval '11 minutes') ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n = 1;
END $$;

-- Binds a new app session to its device key: the account's live row for the key, or a new one.
-- The device's id, or NULL when the session isn't this account's app session or is already bound.
CREATE FUNCTION auth.session_bind(sid text, acct text, key bytea, platform_ text, store_ text, name_ text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE dev bigint;
BEGIN
  PERFORM 1 FROM auth.sessions s WHERE s.id = sid AND s.account = acct AND s.client = 'app' AND s.device IS NULL FOR UPDATE;
  IF NOT FOUND THEN RETURN NULL; END IF;
  SELECT d.id INTO dev FROM auth.devices d WHERE d.account = acct AND d.public_key = key AND d.revoked_at IS NULL;
  IF dev IS NULL THEN
    INSERT INTO auth.devices (account, public_key, platform, store, name, last_seen_at)
    VALUES (acct, key, platform_, store_, nullif(left(name_, 80), ''), now()) RETURNING id INTO dev;
  ELSE
    UPDATE auth.devices SET last_seen_at = now(), name = coalesce(nullif(left(name_, 80), ''), name) WHERE id = dev;
  END IF;
  UPDATE auth.sessions SET device = dev WHERE id = sid;
  RETURN dev;
END $$;

-- What a refresh token is bound to, for the policy: its session's device key, if any.
CREATE FUNCTION auth.refresh_look(rt_sha text) RETURNS TABLE (
  family text, session text, account text, device bigint, public_key text, revoked boolean, now_ms bigint)
LANGUAGE sql STABLE AS $$
  SELECT r.family, s.id, s.account, s.device, coalesce(encode(d.public_key, 'hex'), ''), d.revoked_at IS NOT NULL,
         (extract(epoch FROM now()) * 1000)::bigint
    FROM auth.refresh r JOIN auth.sessions s ON s.id = r.session LEFT JOIN auth.devices d ON d.id = s.device
   WHERE r.token_sha256 = rt_sha
$$;

-- A refresh (0143's), now with the device whose proof the API checked (NULL: none). A bound
-- session's token moves only with its own live device's proof: 'proof' otherwise, and nothing
-- changes, so a copied token can neither refresh nor end the login.
CREATE FUNCTION auth.app_refresh(rt_sha text, new_rt text, new_at text, proved bigint, OUT state text, OUT account text, OUT family text) LANGUAGE plpgsql AS $$
DECLARE r auth.refresh; dev bigint; live boolean;
BEGIN
  SELECT * INTO r FROM auth.refresh WHERE token_sha256 = rt_sha FOR UPDATE;
  IF r.token_sha256 IS NULL THEN state := 'unknown'; RETURN; END IF;
  SELECT s.account, s.device INTO account, dev FROM auth.sessions s WHERE s.id = r.session;
  family := r.family;
  IF dev IS NOT NULL THEN
    SELECT d.revoked_at IS NULL INTO live FROM auth.devices d WHERE d.id = dev;
    IF proved IS DISTINCT FROM dev OR NOT coalesce(live, false) THEN state := 'proof'; RETURN; END IF;
  END IF;
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
  IF dev IS NOT NULL THEN UPDATE auth.devices SET last_seen_at = now() WHERE id = dev; END IF;
  state := 'ok';
END $$;

-- 0143's three-argument refresh stays for the API revision that is serving while this migration
-- runs (it calls it until the new revision takes over): it is the refresh above with no proved
-- device, so it moves an unbound login's token as before and never a bound one's.
CREATE OR REPLACE FUNCTION auth.app_refresh(rt_sha text, new_rt text, new_at text, OUT state text, OUT account text, OUT family text) LANGUAGE sql AS $$
  SELECT r.state, r.account, r.family FROM auth.app_refresh(rt_sha, new_rt, new_at, NULL::bigint) r
$$;

-- The devices list: each live key with its name, platform, where it is kept, when it was last used
-- and how many logins it holds. this: the one this session is bound to.
CREATE FUNCTION auth.devices_of(acct text, this_sid text) RETURNS TABLE (
  id bigint, name text, platform text, store text, created_ms bigint, last_seen_ms bigint, sessions int, this boolean)
LANGUAGE sql STABLE AS $$
  SELECT d.id, coalesce(d.name, CASE d.platform WHEN 'ios' THEN 'iPhone' WHEN 'mac' THEN 'Mac' WHEN 'android' THEN 'Android' ELSE 'Browser' END),
         d.platform, d.store,
         (extract(epoch FROM d.created_at) * 1000)::bigint,
         (extract(epoch FROM coalesce(d.last_seen_at, d.created_at)) * 1000)::bigint,
         (SELECT count(*) FROM auth.sessions s WHERE s.device = d.id)::int,
         EXISTS (SELECT 1 FROM auth.sessions s WHERE s.device = d.id AND s.id = this_sid)
    FROM auth.devices d WHERE d.account = acct AND d.revoked_at IS NULL
   ORDER BY coalesce(d.last_seen_at, d.created_at) DESC, d.id DESC
$$;
-- One device, for the policy: whether it is this account's and whether it was removed already.
CREATE FUNCTION auth.device_look(acct text, dev bigint, OUT found boolean, OUT mine boolean, OUT removed boolean) LANGUAGE sql STABLE AS $$
  SELECT count(*) > 0, coalesce(bool_or(d.account = acct), false), coalesce(bool_or(d.revoked_at IS NOT NULL), false) FROM auth.devices d WHERE d.id = dev
$$;
-- Remove: the key is revoked and every login bound to it ends now. How many ended; NULL when it
-- isn't this account's live device.
CREATE FUNCTION auth.device_remove(acct text, dev bigint) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.devices SET revoked_at = now() WHERE id = dev AND account = acct AND revoked_at IS NULL;
  IF NOT FOUND THEN RETURN NULL; END IF;
  DELETE FROM auth.sessions WHERE device = dev;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;
-- The sessions list (0143's) also says which device key a login is bound to.
CREATE OR REPLACE FUNCTION auth.sessions_of(acct text, this_sid text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object(
           'id', s.id, 'name', coalesce(s.name, CASE s.client WHEN 'app' THEN 'App' ELSE 'Browser' END),
           'client', s.client, 'via', s.via_kind, 'level', s.level, 'country', s.country,
           'created_ms', (extract(epoch FROM s.created_at) * 1000)::bigint,
           'last_seen_ms', (extract(epoch FROM s.last_seen_at) * 1000)::bigint,
           'this', s.id = this_sid, 'device', s.device) ORDER BY s.last_seen_at DESC), '[]'::jsonb)
    FROM auth.sessions s WHERE s.account = acct
$$;

-- ---- a device key is not a stronger way in (see the top)
CREATE OR REPLACE FUNCTION auth.strong_ways(acct text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT (SELECT count(*) FROM auth.nostr_keys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.passkeys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.recovery WHERE account = acct AND spent_at IS NULL)::int
$$;
CREATE OR REPLACE FUNCTION auth.methods_of(acct text) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT array_remove(ARRAY[
    CASE WHEN EXISTS (SELECT 1 FROM auth.nostr_keys WHERE account = acct) THEN 'nostr' END,
    CASE WHEN EXISTS (SELECT 1 FROM auth.passkeys WHERE account = acct) THEN 'passkey' END,
    CASE WHEN EXISTS (SELECT 1 FROM auth.recovery WHERE account = acct AND spent_at IS NULL) THEN 'recovery_code' END], NULL)
$$;
-- auth.life_of (0149) with the same count.
CREATE OR REPLACE FUNCTION auth.life_of(acct text) RETURNS TABLE (
  kind text, frozen_ms bigint, frozen_why text, delete_asked_ms bigint, purge_ms bigint, now_ms bigint,
  strong int, emails int, keys int, agents int, agents_paused int, credit_msat bigint, worlds int, saves int,
  merged_from int, deleted boolean, merged_into text) LANGUAGE sql STABLE AS $$
  SELECT a.kind,
         coalesce((extract(epoch FROM a.frozen_at) * 1000)::bigint, 0), coalesce(a.frozen_why, ''),
         coalesce((extract(epoch FROM a.delete_asked_at) * 1000)::bigint, 0),
         coalesce((extract(epoch FROM a.purge_at) * 1000)::bigint, 0),
         (extract(epoch FROM now()) * 1000)::bigint,
         auth.strong_ways(a.id),
         (SELECT count(*) FROM auth.emails e WHERE e.account = a.id)::int,
         (SELECT count(*) FROM auth.nostr_keys k WHERE k.account = a.id)::int,
         (SELECT count(*) FROM auth.accounts g WHERE g.owner = a.id AND g.deleted_at IS NULL)::int,
         (SELECT count(*) FROM auth.agent_grants g WHERE g.owner = a.id AND g.revoked_at IS NULL AND g.suspended_at IS NOT NULL)::int,
         coalesce((SELECT sum(c.msat) FROM inference.credit c JOIN auth.nostr_keys k ON k.pubkey = c.pubkey WHERE k.account = a.id AND c.msat > 0), 0)::bigint,
         (SELECT count(*) FROM verse.worlds w WHERE w.owner_account = a.verse_account)::int,
         (SELECT count(*) FROM verse.progress g WHERE g.account = a.verse_account)::int,
         (SELECT count(*) FROM auth.merges m WHERE m.survivor = a.id)::int,
         a.deleted_at IS NOT NULL, coalesce(a.merged_into, '')
    FROM auth.accounts a WHERE a.id = acct
$$;

-- Roles (#13194): the API reads and writes rows; the dump and read-only roles read.
DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON auth.device_proofs TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT ON auth.device_proofs TO %I', r);
    END IF;
  END LOOP;
END $$;
