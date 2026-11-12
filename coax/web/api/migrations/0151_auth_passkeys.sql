-- Passkeys (WebAuthn; docs/auth/identity-and-methods.md#passkeys-webauthn, #13215). auth.passkeys
-- came with the schema (0142); this adds what the ceremonies need. What may happen is decided in
-- passkey-policy.coax; webauthn.coax checks the browser's answers; these rows hold the facts:
--
--   * a challenge is an auth.pending row of kind 'passkey' (purpose 'login', 'link' for adding a
--     passkey, or 'stepup'), kept as its SHA-256, made by the API and used once: auth.passkey_spend
--     marks it used in the same statement that reads it, so a second try finds nothing
--   * a login challenge belongs to the browser that asked (browser_sha256, the __Host-oa_passkey
--     cookie's digest) or to the app that holds it; adding a passkey and stepping up belong to one
--     session of one account (account, session)
--   * a passkey's user handle is 32 random bytes per account (every passkey of an account shares
--     it), never the account id or an email
--   * clone_at: when a login showed a sign count that went backwards (a copied passkey); the
--     login was refused and the account told
--   * rp_id: the relying party it was made for (openagents.com, or staging.openagents.com)
ALTER TABLE auth.pending DROP CONSTRAINT pending_kind_check;
ALTER TABLE auth.pending ADD CONSTRAINT pending_kind_check CHECK (kind IN ('email', 'nostr', 'phone', 'passkey'));
ALTER TABLE auth.pending ADD COLUMN session text;   -- the session a passkey's 'link' or 'stepup' challenge belongs to

ALTER TABLE auth.passkeys
  ADD COLUMN rp_id text,
  ADD COLUMN prf boolean NOT NULL DEFAULT false,   -- the authenticator said it can do PRF (the wallet backup's slot, #13217)
  ADD COLUMN clone_at timestamptz,
  ADD CONSTRAINT passkeys_alg CHECK (alg IN (-7, -8, -257)),
  ADD CONSTRAINT passkeys_name CHECK (name IS NULL OR length(name) BETWEEN 1 AND 64),
  ADD CONSTRAINT passkeys_handle CHECK (user_handle ~ '^[A-Za-z0-9_-]{43}$'),
  ADD CONSTRAINT passkeys_credential CHECK (credential_id ~ '^[A-Za-z0-9_-]+$' AND length(credential_id) BETWEEN 2 AND 1400),
  ADD CONSTRAINT passkeys_salt CHECK (prf_salt IS NULL OR length(prf_salt) = 32);
ALTER TABLE auth.accounts ADD COLUMN passkey_offered_at timestamptz;   -- the one offer after an email login

-- ---- challenges
CREATE FUNCTION auth.passkey_challenge(id_ text, challenge_sha text, purpose_ text, client_ text, acct text, sid text, browser_sha text)
RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.pending (id, kind, purpose, link_sha256, client, account, session, browser_sha256, expires_at)
  VALUES (id_, 'passkey', purpose_, challenge_sha, client_, acct, sid, browser_sha, now() + interval '5 minutes')
$$;
-- Uses a challenge: marks it done and answers its facts (no row when there is no unused one).
-- passkey-policy.coax's pk_challenge decides from them.
CREATE FUNCTION auth.passkey_spend(challenge_sha text) RETURNS TABLE (
  purpose text, client text, account text, session text, browser_sha256 text, made_ms bigint, now_ms bigint)
LANGUAGE sql AS $$
  UPDATE auth.pending p SET done_at = now()
   WHERE p.link_sha256 = challenge_sha AND p.kind = 'passkey' AND p.done_at IS NULL
  RETURNING p.purpose, p.client, p.account, p.session, p.browser_sha256,
            (extract(epoch FROM p.created_at) * 1000)::bigint, (extract(epoch FROM now()) * 1000)::bigint
$$;

-- ---- passkeys
-- The account's user handle: the one its passkeys share, or NULL for none yet.
CREATE FUNCTION auth.passkey_handle(acct text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT user_handle FROM auth.passkeys WHERE account = acct ORDER BY id LIMIT 1
$$;
-- 0 nobody holds this credential, 1 this account, 2 another.
CREATE FUNCTION auth.passkey_owner(cred text, acct text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT CASE WHEN account = acct THEN 1 ELSE 2 END FROM auth.passkeys WHERE credential_id = cred), 0)
$$;
CREATE FUNCTION auth.passkey_add(acct text, cred text, handle text, cose_hex text, alg_ int, count_ bigint, be boolean, bs boolean,
                                 aaguid_ text, transports_ text[], name_ text, salt_hex text, prf_ boolean, rp text)
RETURNS bigint LANGUAGE sql AS $$
  INSERT INTO auth.passkeys (account, credential_id, user_handle, public_key, alg, sign_count, backup_eligible, backed_up,
                             aaguid, transports, name, prf_salt, prf, rp_id, last_used_at)
  VALUES (acct, cred, handle, decode(cose_hex, 'hex'), alg_, count_, be, bs, nullif(aaguid_, ''), transports_, name_,
          decode(salt_hex, 'hex'), prf_, rp, now())
  RETURNING id
$$;
-- What a login needs about a credential: its row, key and count, and its account's state.
CREATE FUNCTION auth.passkey_find(cred text, rp text) RETURNS TABLE (
  id bigint, account text, user_handle text, cose_hex text, sign_count bigint, backup_eligible boolean, frozen boolean)
LANGUAGE sql STABLE AS $$
  SELECT p.id, p.account, p.user_handle, encode(p.public_key, 'hex'), p.sign_count, p.backup_eligible, a.frozen_at IS NOT NULL
    FROM auth.passkeys p JOIN auth.accounts a ON a.id = p.account
   WHERE p.credential_id = cred AND coalesce(p.rp_id, rp) = rp AND a.deleted_at IS NULL AND a.merged_into IS NULL
$$;
-- A good login: the new count, whether it is backed up now, when.
CREATE FUNCTION auth.passkey_used(id_ bigint, count_ bigint, bs boolean) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.passkeys SET sign_count = count_, backed_up = bs, last_used_at = now() WHERE id = id_
$$;
CREATE FUNCTION auth.passkey_clone(id_ bigint) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.passkeys SET clone_at = now() WHERE id = id_
$$;
CREATE FUNCTION auth.passkey_rename(acct text, id_ bigint, name_ text) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.passkeys SET name = name_ WHERE id = id_ AND account = acct RETURNING 1) SELECT EXISTS (SELECT 1 FROM u)
$$;
-- The Security page's list: never a key, a handle or a salt.
CREATE FUNCTION auth.passkeys_of(acct text) RETURNS TABLE (id bigint, name text, added_ms bigint, used_ms bigint, synced boolean,
  clone boolean, prf boolean, transports text)
LANGUAGE sql STABLE AS $$
  SELECT p.id, coalesce(p.name, 'Passkey'), (extract(epoch FROM p.created_at) * 1000)::bigint,
         coalesce((extract(epoch FROM p.last_used_at) * 1000)::bigint, 0), p.backed_up, p.clone_at IS NOT NULL, p.prf,
         coalesce(array_to_string(p.transports, ','), '')
    FROM auth.passkeys p WHERE p.account = acct ORDER BY p.id
$$;
-- The offer after an email login: shown once (the page marks it).
CREATE FUNCTION auth.passkey_offered(acct text) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.accounts SET passkey_offered_at = coalesce(passkey_offered_at, now()) WHERE id = acct
$$;
