-- Our own accounts (docs/auth/, #13200): one account, many authenticators, in our own Postgres.
-- No identity provider holds a copy, and there is no phone table or phone column anywhere (D3).
--
-- An account id is acct_ plus 26 base32 characters of random bits; it never encodes a key, an
-- email or a provider, so adding or removing an authenticator moves no data. One authenticator
-- belongs to exactly one account (the unique indexes). Every secret is a digest; an email is
-- sealed by the API (XChaCha20-Poly1305 under oa-auth-seal-key, auth-seal.coax) before it reaches
-- this database, and found again by a keyed hash (HMAC-SHA256 under oa-auth-lookup-pepper). Both
-- carry a key version, so a new key is added rather than swapped.
--
-- verse_account is the game's account (verse.accounts.id) the account plays as: a new account's
-- own id, or, for a Nostr key that already had realms and saves under the old n-<pubkey>, that
-- id, so nothing anyone made moves. auth.account_of keeps answering the game's account for a key.
--
-- The tables of the WorkOS-era groundwork (0090) whose names these take are renamed out of the
-- way; #13207 drops them with the rest of the old login code.
ALTER TABLE auth.sessions RENAME TO v0_sessions;
-- Its two functions name the table in their bodies, read when they run: they move with it.
CREATE OR REPLACE FUNCTION auth.workos_link(profile jsonb, session_pk text) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  INSERT INTO auth.v0_sessions VALUES (projects.sha(t), acct, 'workos', projects.now_ms(), projects.now_ms() + 30 * 86400000::bigint);
  RETURN access.r(200, jsonb_build_object('account', acct, 'token', t), NULL);
END $$;
CREATE OR REPLACE FUNCTION auth.session(t text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.account FROM auth.v0_sessions s WHERE s.token_sha256 = projects.sha(coalesce(t, '')) AND s.expires_ms > projects.now_ms()
$$;


-- 26 base32 characters (RFC 4648 alphabet, lower case) of 130 random bits.
CREATE FUNCTION auth.random_b32(n int) RETURNS text LANGUAGE plpgsql VOLATILE AS $$
DECLARE
  bits bytea := sha256(convert_to(gen_random_uuid()::text || gen_random_uuid()::text || clock_timestamp()::text, 'UTF8'));
  alphabet text := 'abcdefghijklmnopqrstuvwxyz234567';
  out text := '';
  i int;
  v int;
BEGIN
  FOR i IN 0..n - 1 LOOP
    -- five bits starting at bit 5i
    v := ((get_byte(bits, (5 * i) / 8) << 8) | get_byte(bits, (5 * i) / 8 + 1)) >> (11 - (5 * i) % 8) & 31;
    out := out || substr(alphabet, v + 1, 1);
  END LOOP;
  RETURN out;
END $$;
CREATE FUNCTION auth.new_account_id() RETURNS text LANGUAGE sql VOLATILE AS $$ SELECT 'acct_' || auth.random_b32(26) $$;

CREATE TABLE auth.accounts (
  id text PRIMARY KEY CHECK (id ~ '^acct_[a-z2-7]{26}$'),
  kind text NOT NULL CHECK (kind IN ('person', 'agent', 'service')),
  owner text REFERENCES auth.accounts (id),
  display_name text CHECK (length(display_name) <= 80),
  team boolean NOT NULL DEFAULT false,   -- our own testing: never counted as usage
  verse_account text NOT NULL UNIQUE,    -- the game's account it plays as (see above)
  frozen_at timestamptz,
  session_epoch int NOT NULL DEFAULT 0,  -- bumped by "log out everywhere": older sessions are refused
  created_at timestamptz NOT NULL DEFAULT now(),
  deleted_at timestamptz,
  merged_into text REFERENCES auth.accounts (id),
  CHECK ((kind = 'agent') = (owner IS NOT NULL)),
  CHECK (kind <> 'service' OR owner IS NULL)
);
-- An agent's owner is a person (a check can't see another row; this trigger can).
CREATE FUNCTION auth.owner_is_person() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.owner IS NOT NULL AND NOT EXISTS (SELECT 1 FROM auth.accounts WHERE id = NEW.owner AND kind = 'person') THEN
    RAISE EXCEPTION 'auth: an agent''s owner must be a person' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER owner_is_person BEFORE INSERT OR UPDATE OF owner, kind ON auth.accounts FOR EACH ROW EXECUTE FUNCTION auth.owner_is_person();

CREATE TABLE auth.emails (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  lookup text NOT NULL UNIQUE CHECK (lookup ~ '^[0-9]+:[0-9a-f]{64}$'),   -- key version : HMAC-SHA256
  sealed text NOT NULL,                  -- base64: version byte, 24-byte nonce, ciphertext and tag
  verified_at timestamptz,
  is_primary boolean NOT NULL DEFAULT false,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz
);
CREATE INDEX emails_account ON auth.emails (account);
CREATE TABLE auth.nostr_keys (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  pubkey text NOT NULL UNIQUE CHECK (pubkey ~ '^[0-9a-f]{64}$'),
  label text CHECK (length(label) <= 80),   -- phone, extension, bunker, cli:<machine>
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz
);
CREATE INDEX nostr_keys_account ON auth.nostr_keys (account);
CREATE TABLE auth.passkeys (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  credential_id text NOT NULL UNIQUE,    -- base64url
  user_handle text NOT NULL,             -- 32 random bytes per account, never the account id
  public_key bytea NOT NULL,             -- COSE
  alg int NOT NULL,
  sign_count bigint NOT NULL DEFAULT 0,
  backup_eligible boolean NOT NULL DEFAULT false,
  backed_up boolean NOT NULL DEFAULT false,
  aaguid text,
  transports text[],
  name text,
  prf_salt bytea,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_used_at timestamptz
);
CREATE INDEX passkeys_account ON auth.passkeys (account);
CREATE TABLE auth.devices (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  public_key bytea NOT NULL UNIQUE,      -- P-256, from the Secure Enclave, Keystore or WebCrypto
  platform text NOT NULL CHECK (platform IN ('ios', 'mac', 'android', 'web')),
  name text,
  created_at timestamptz NOT NULL DEFAULT now(),
  last_seen_at timestamptz,
  revoked_at timestamptz
);
CREATE TABLE auth.recovery (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  code_scrypt bytea NOT NULL,            -- scrypt(words, salt, N=2^17, r=8, p=1)
  salt bytea NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  spent_at timestamptz
);
CREATE TABLE auth.recovery_requests (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  asked_at timestamptz NOT NULL DEFAULT now(),
  ready_at timestamptz NOT NULL,         -- 72 hours later (D10)
  cancelled_at timestamptz,
  done_at timestamptz
);
-- A login (or a link) waiting to be proved: an email code, a Nostr challenge, a phone QR.
CREATE TABLE auth.pending (
  id text PRIMARY KEY,                   -- 32 hex
  kind text NOT NULL CHECK (kind IN ('email', 'nostr', 'phone')),
  purpose text NOT NULL DEFAULT 'login' CHECK (purpose IN ('login', 'link')),
  code_sha256 text,                      -- SHA-256(code || id)
  link_sha256 text UNIQUE,               -- the email link's token, or the phone QR's challenge
  poll_sha256 text UNIQUE,               -- the phone QR page's poll token
  target_lookup text,                    -- the email's lookup
  target_sealed text,                    -- the email, sealed with this row's id as its associated data
  account text REFERENCES auth.accounts (id),   -- a link's account; a done login's account
  browser_sha256 text,                   -- SHA-256 of the __Host-oa_pending cookie that asked
  client text NOT NULL DEFAULT 'web' CHECK (client IN ('web', 'app')),
  tries int NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL,
  approved_pubkey text,                  -- the phone QR: the key that approved it
  done_at timestamptz
);
CREATE INDEX pending_expires ON auth.pending (expires_at);
CREATE TABLE auth.nip98_seen (
  event_id text PRIMARY KEY CHECK (event_id ~ '^[0-9a-f]{64}$'),
  expires_at timestamptz NOT NULL        -- kept ten minutes past its window
);
CREATE TABLE auth.connections (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  provider text NOT NULL CHECK (provider IN ('github', 'google')),
  subject text NOT NULL,
  scopes text[] NOT NULL DEFAULT '{}',
  token_sealed text,
  refreshed_at timestamptz,
  UNIQUE (provider, subject)
);

-- Ways to log in: an email, a Nostr key, a passkey, an unspent recovery code.
CREATE FUNCTION auth.login_ways(acct text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT (SELECT count(*) FROM auth.emails WHERE account = acct)::int
       + (SELECT count(*) FROM auth.nostr_keys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.passkeys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.recovery WHERE account = acct AND spent_at IS NULL)::int
$$;
-- Removing an account's last way in is refused ("Add another way to log in first"), unless the
-- account itself is being deleted or merged.
CREATE FUNCTION auth.keep_a_way_in() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM auth.accounts a WHERE a.id = OLD.account AND a.deleted_at IS NULL AND a.merged_into IS NULL)
     AND auth.login_ways(OLD.account) = 0 THEN
    RAISE EXCEPTION 'Add another way to log in first' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER keep_a_way_in AFTER DELETE ON auth.emails DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION auth.keep_a_way_in();
CREATE CONSTRAINT TRIGGER keep_a_way_in AFTER DELETE ON auth.nostr_keys DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION auth.keep_a_way_in();
CREATE CONSTRAINT TRIGGER keep_a_way_in AFTER DELETE ON auth.passkeys DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION auth.keep_a_way_in();
CREATE CONSTRAINT TRIGGER keep_a_way_in AFTER DELETE OR UPDATE OF spent_at ON auth.recovery DEFERRABLE INITIALLY IMMEDIATE FOR EACH ROW EXECUTE FUNCTION auth.keep_a_way_in();

-- Our team (team.conf, admins.conf: analytics.team): its accounts' visits are testing, not usage.
CREATE FUNCTION auth.is_team(acct text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM auth.nostr_keys k JOIN analytics.team t ON t.kind = 'npub' AND t.value = k.pubkey WHERE k.account = acct)
$$;

-- A new person's account. verse_account: the game's account it plays as (its own id when NULL).
CREATE FUNCTION auth.new_person(verse_acct text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE id_ text := auth.new_account_id();
BEGIN
  INSERT INTO verse.accounts (id, created_ms) VALUES (coalesce(verse_acct, id_), verse.now_ms()) ON CONFLICT DO NOTHING;
  INSERT INTO auth.accounts (id, kind, verse_account) VALUES (id_, 'person', coalesce(verse_acct, id_));
  RETURN id_;
END $$;

-- The account a Nostr key logs in to, made on its first login. A key that played before under
-- n-<pubkey> (or that the old groundwork linked to another account) keeps playing as that account.
-- made is true when this call made it.
CREATE FUNCTION auth.account_for_key(pk text, label_ text, OUT acct text, OUT made boolean) LANGUAGE plpgsql AS $$
DECLARE old text;
BEGIN
  made := false;
  IF coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN RAISE EXCEPTION 'auth: not a key' USING ERRCODE = 'invalid_parameter_value'; END IF;
  SELECT account INTO acct FROM auth.nostr_keys WHERE pubkey = pk;
  IF acct IS NOT NULL THEN
    UPDATE auth.nostr_keys SET last_used_at = now() WHERE pubkey = pk;
    RETURN;
  END IF;
  old := coalesce((SELECT i.account FROM auth.identities i WHERE i.kind = 'nostr' AND i.subject = pk),
                  (SELECT a.id FROM verse.accounts a WHERE a.id = 'n-' || pk));
  -- An old account already adopted by another key keeps its one owner: this key starts fresh.
  IF old IS NOT NULL AND EXISTS (SELECT 1 FROM auth.accounts WHERE verse_account = old) THEN old := NULL; END IF;
  acct := auth.new_person(old);
  INSERT INTO auth.nostr_keys (account, pubkey, label, last_used_at) VALUES (acct, pk, nullif(left(label_, 80), ''), now());
  UPDATE auth.accounts SET team = auth.is_team(acct) WHERE id = acct;
  made := true;
END $$;

-- The game's account a key or an account id acts as (realms, players, the invite gate):
--   an account id (acct_...)      that account's
--   a key on an account           that account's
--   any other key                 n-<pubkey>, as before: a CLI or agent key no one linked
-- Replaces 0090's, which read the old groundwork's links.
CREATE OR REPLACE FUNCTION auth.account_of(pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE
    WHEN pk ~ '^acct_[a-z2-7]{26}$' THEN (SELECT a.verse_account FROM auth.accounts a WHERE a.id = pk AND a.deleted_at IS NULL)
    WHEN coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN NULL
    ELSE coalesce((SELECT a.verse_account FROM auth.nostr_keys k JOIN auth.accounts a ON a.id = k.account WHERE k.pubkey = pk),
                  (SELECT i.account FROM auth.identities i WHERE i.kind = 'nostr' AND i.subject = pk),
                  'n-' || pk) END
$$;

-- The invite gate (docs/auth/2026-10-31-realm-invites-and-auth.md §4) on these accounts: a creator,
-- request or redemption names the game account auth.account_of answers, so a person who logs in by
-- email or by any of their keys is the same creator. Nothing else about the gate changes.
COMMENT ON TABLE access.creators IS 'who: auth.account_of(the caller) (0142): an account''s verse_account';
