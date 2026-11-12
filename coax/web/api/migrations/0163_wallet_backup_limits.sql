-- The wallet's encrypted seed backup and its daily spend limit (docs/auth/wallets-and-custody.md,
-- decision D4, #13217). What may happen is decided in wallet-policy.coax (#authority); these rows
-- hold the facts, and the constraints and functions below refuse on their own what the policy
-- must never allow.
--
-- auth.wallet_backups holds what a person's device sealed: NIP-VAULT key slots and one object.
-- Every column is ciphertext or public: the object is AES-256-GCM under a key made on the device;
-- each slot is that key's key under a secret we never receive (a passkey's PRF output, scrypt of
-- a recovery code made on the device, a Nostr key's own seal). There is no column here, or
-- anywhere, for a PRF output, a recovery code, a hash of one, or a key. The row is replaced in
-- place: an old epoch isn't kept (NIP-VAULT, "Key index": no soft delete, no versions).
--
--   * an epoch only ever goes up by one (wallet_backup_put), so an old backup can't be written back
--   * a raise of the limit never counts sooner than 24 hours after it was asked (raise_waits_24_hours)
--   * a waiting raise is always more than the limit it would replace (raise_is_more)
--   * a payment id counts once for an account (wallet_spends' key)

CREATE TABLE auth.wallet_backups (
  account text PRIMARY KEY REFERENCES auth.accounts (id),
  vault text NOT NULL CHECK (vault ~ '^[0-9a-f]{64}$'),
  epoch bigint NOT NULL CHECK (epoch >= 1),
  slots text NOT NULL CHECK (length(slots) BETWEEN 1 AND 24576),      -- each slot's own bytes, a newline between them
  object bytea NOT NULL CHECK (length(object) BETWEEN 28 AND 4096),   -- OAVAULT1, its header, one sealed chunk
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

-- Stores a backup when it follows the stored epoch (after: the epoch the writer saw; 0: none yet).
-- The new epoch, or NULL when another write came first.
CREATE FUNCTION auth.wallet_backup_put(acct text, after bigint, vault_ text, slots_ text, object_ bytea) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE cur bigint;
BEGIN
  PERFORM 1 FROM auth.accounts a WHERE a.id = acct FOR UPDATE;
  SELECT b.epoch INTO cur FROM auth.wallet_backups b WHERE b.account = acct;
  IF coalesce(cur, 0) <> after THEN RETURN NULL; END IF;
  IF cur IS NULL THEN
    INSERT INTO auth.wallet_backups (account, vault, epoch, slots, object) VALUES (acct, vault_, 1, slots_, object_);
    RETURN 1;
  END IF;
  UPDATE auth.wallet_backups SET vault = vault_, epoch = cur + 1, slots = slots_, object = object_, updated_at = now() WHERE account = acct;
  RETURN cur + 1;
END $$;
-- An epoch never goes back and never skips, whatever writes the row.
CREATE FUNCTION auth.wallet_backup_forward() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.epoch <> OLD.epoch + 1 THEN
    RAISE EXCEPTION 'auth.wallet_backups: an epoch goes up by one' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER wallet_backup_forward BEFORE UPDATE OF epoch, slots, object, vault ON auth.wallet_backups
  FOR EACH ROW EXECUTE FUNCTION auth.wallet_backup_forward();

CREATE FUNCTION auth.wallet_backup_of(acct text) RETURNS TABLE (vault text, epoch bigint, slots text, object_hex text, created_ms bigint, updated_ms bigint)
LANGUAGE sql STABLE AS $$
  SELECT b.vault, b.epoch, b.slots, encode(b.object, 'hex'), (extract(epoch FROM b.created_at) * 1000)::bigint, (extract(epoch FROM b.updated_at) * 1000)::bigint
    FROM auth.wallet_backups b WHERE b.account = acct
$$;
-- Deletes the backup: 1 when there was one. The next backup starts again at epoch 1 in a new vault.
CREATE FUNCTION auth.wallet_backup_drop(acct text) RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.wallet_backups WHERE account = acct RETURNING 1) SELECT count(*)::int FROM d
$$;

-- ---- the daily limit
CREATE TABLE auth.wallet_limits (
  account text PRIMARY KEY REFERENCES auth.accounts (id),
  daily_sat bigint NOT NULL CHECK (daily_sat BETWEEN 0 AND 2100000000000000),
  pending_sat bigint,                    -- a raise that is waiting
  pending_asked_at timestamptz,
  pending_at timestamptz,                -- when it counts
  cancel_sha256 text CHECK (cancel_sha256 ~ '^[0-9a-f]{64}$'),   -- the notice's cancel link, its digest only
  updated_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT raise_whole CHECK ((pending_sat IS NULL) = (pending_at IS NULL) AND (pending_sat IS NULL) = (pending_asked_at IS NULL)
                                AND (pending_sat IS NOT NULL OR cancel_sha256 IS NULL)),
  CONSTRAINT raise_is_more CHECK (pending_sat IS NULL OR (pending_sat > daily_sat AND pending_sat <= 2100000000000000)),
  CONSTRAINT raise_waits_24_hours CHECK (pending_at IS NULL OR pending_at >= pending_asked_at + interval '24 hours')
);

-- The limit's facts for the policy: the stored limit, the waiting raise (0, 0 when none) and now.
-- A raise whose time has come is made the limit first, so a row never holds a raise that counts.
CREATE FUNCTION auth.wallet_limit_of(acct text, default_sat bigint, OUT daily_sat bigint, OUT pending_sat bigint, OUT pending_asked_ms bigint,
                                     OUT pending_ms bigint, OUT now_ms bigint) LANGUAGE plpgsql AS $$
BEGIN
  UPDATE auth.wallet_limits l SET daily_sat = l.pending_sat, pending_sat = NULL, pending_asked_at = NULL, pending_at = NULL, cancel_sha256 = NULL, updated_at = now()
   WHERE l.account = acct AND l.pending_at IS NOT NULL AND l.pending_at <= now();
  SELECT l.daily_sat, coalesce(l.pending_sat, 0), coalesce((extract(epoch FROM l.pending_asked_at) * 1000)::bigint, 0),
         coalesce((extract(epoch FROM l.pending_at) * 1000)::bigint, 0)
    INTO daily_sat, pending_sat, pending_asked_ms, pending_ms FROM auth.wallet_limits l WHERE l.account = acct;
  IF daily_sat IS NULL THEN daily_sat := default_sat; pending_sat := 0; pending_asked_ms := 0; pending_ms := 0; END IF;
  now_ms := (extract(epoch FROM now()) * 1000)::bigint;
END $$;
-- A lower limit, now. Any waiting raise ends with it.
CREATE FUNCTION auth.wallet_limit_lower(acct text, sat bigint) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.wallet_limits (account, daily_sat) VALUES (acct, sat)
  ON CONFLICT (account) DO UPDATE SET daily_sat = EXCLUDED.daily_sat, pending_sat = NULL, pending_asked_at = NULL, pending_at = NULL, cancel_sha256 = NULL, updated_at = now()
$$;
-- A raise that waits: it counts 24 hours from now, whatever raise was waiting before. current: the
-- limit in force (the default when the account has no row yet). When it counts, in milliseconds.
CREATE FUNCTION auth.wallet_limit_raise(acct text, current bigint, sat bigint, cancel text) RETURNS bigint LANGUAGE sql AS $$
  INSERT INTO auth.wallet_limits (account, daily_sat, pending_sat, pending_asked_at, pending_at, cancel_sha256)
  VALUES (acct, current, sat, now(), now() + interval '24 hours', cancel)
  ON CONFLICT (account) DO UPDATE SET pending_sat = EXCLUDED.pending_sat, pending_asked_at = now(), pending_at = now() + interval '24 hours',
                                      cancel_sha256 = EXCLUDED.cancel_sha256, updated_at = now()
  RETURNING (extract(epoch FROM pending_at) * 1000)::bigint
$$;
-- Cancels the waiting raise: the sats it would have raised to, or NULL when none was waiting.
CREATE FUNCTION auth.wallet_limit_cancel(acct text) RETURNS bigint LANGUAGE sql AS $$
  WITH old AS (SELECT l.pending_sat FROM auth.wallet_limits l WHERE l.account = acct AND l.pending_at > now() FOR UPDATE),
       u AS (UPDATE auth.wallet_limits l SET pending_sat = NULL, pending_asked_at = NULL, pending_at = NULL, cancel_sha256 = NULL, updated_at = now()
              WHERE l.account = acct AND EXISTS (SELECT 1 FROM old) RETURNING 1)
  SELECT (SELECT pending_sat FROM old) FROM (SELECT count(*) FROM u) done
$$;
-- The account whose waiting raise this cancel link is for, or NULL.
CREATE FUNCTION auth.wallet_limit_by_link(sha text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT l.account FROM auth.wallet_limits l WHERE l.cancel_sha256 = sha AND l.pending_at > now()
$$;

-- ---- what was spent
CREATE TABLE auth.wallet_spends (
  account text NOT NULL REFERENCES auth.accounts (id),
  payment_id text NOT NULL CHECK (payment_id ~ '^[A-Za-z0-9_-]{8,64}$'),
  sats bigint NOT NULL CHECK (sats BETWEEN 1 AND 2100000000000000),
  at timestamptz NOT NULL DEFAULT now(),
  voided_at timestamptz,                 -- the payment didn't go: it no longer counts
  PRIMARY KEY (account, payment_id)
);
CREATE INDEX wallet_spends_recent ON auth.wallet_spends (account, at) WHERE voided_at IS NULL;
-- Sats counted in the last 24 hours, and the sats already recorded under this payment id (0: none).
CREATE FUNCTION auth.wallet_spent(acct text, pid text, OUT spent bigint, OUT seen bigint) LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT sum(s.sats) FROM auth.wallet_spends s WHERE s.account = acct AND s.voided_at IS NULL AND s.at > now() - interval '24 hours'), 0)::bigint,
         coalesce((SELECT s.sats FROM auth.wallet_spends s WHERE s.account = acct AND s.payment_id = pid), 0)::bigint
$$;
CREATE FUNCTION auth.wallet_spend_add(acct text, pid text, sats_ bigint) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  INSERT INTO auth.wallet_spends (account, payment_id, sats) VALUES (acct, pid, sats_) ON CONFLICT DO NOTHING;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n = 1;
END $$;
-- The payment didn't go: true when it was counted and now isn't.
CREATE FUNCTION auth.wallet_spend_void(acct text, pid text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.wallet_spends SET voided_at = now() WHERE account = acct AND payment_id = pid AND voided_at IS NULL;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n = 1;
END $$;
-- Rows older than 30 days say nothing the limit reads.
CREATE FUNCTION auth.wallet_spends_sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.wallet_spends WHERE at < now() - interval '30 days' RETURNING 1) SELECT count(*)::int FROM d
$$;

-- ---- deletion, merging and export (0149's registry). A backup stays with the account that made
-- it: its sealed record names that account, and the apps refuse a record for another one, so a
-- merged account's backup isn't moved (the person backs up again under the account they keep).
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('auth.wallet_backups', 'account', 'account', 'keep', 'delete', true, '{slots,object}', 'the wallet''s encrypted backup (we can''t open it; the export says it exists, since when and its size)'),
  ('auth.wallet_limits', 'account', 'account', 'move', 'delete', true, '{cancel_sha256}', 'the wallet''s daily limit'),
  ('auth.wallet_spends', 'account', 'account', 'move', 'delete', true, '{}', 'payments counted against the daily limit');
