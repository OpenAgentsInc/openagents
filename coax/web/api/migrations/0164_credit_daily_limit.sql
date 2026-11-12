-- Service credit counts toward the wallet's daily limit (docs/auth/wallets-and-custody.md, "Step-up
-- for spends"; #13330). A model call paid from credit (inference.finish, one debit a call, msat a
-- Nostr key) adds its msat to the account that holds the key, in the same transaction as the
-- debit. What a call may do is wallet-policy.coax's (wp_total, wp_credit; #authority): these rows
-- hold the facts.
--
-- The count is a running sum an hour: one row an account an hour, so reading the last 24 hours
-- reads at most 25 rows, whatever the number of calls. An hour's row counts while any of it is in
-- the last 24 hours, so a call counts for 24 to 25 hours: never less than the limit's window.

CREATE TABLE auth.credit_spends (
  account text NOT NULL REFERENCES auth.accounts (id),
  hour timestamptz NOT NULL,                       -- the hour's start (date_trunc('hour', now()))
  msat bigint NOT NULL CHECK (msat > 0),
  PRIMARY KEY (account, hour)
);

-- The msat of service credit the account's model calls took in the last 24 hours.
CREATE FUNCTION auth.credit_spent(acct text) RETURNS bigint LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(c.msat), 0)::bigint FROM auth.credit_spends c WHERE c.account = acct AND c.hour > now() - interval '25 hours'
$$;

-- The facts a model call paid from credit is decided on, by its key: the person's account that holds
-- it, the limit's row (default_sat when there is none), what the last 24 hours counted (the
-- wallet's sats and the credit's msat apart) and how long ago any login of the account proved L2
-- (-1: never). No row: no person's account holds the key.
CREATE FUNCTION auth.credit_limit_facts(pk text, default_sat bigint, OUT account text, OUT daily_sat bigint, OUT pending_sat bigint,
                                        OUT pending_ms bigint, OUT now_ms bigint, OUT wallet_sat bigint, OUT credit_msat bigint,
                                        OUT l2_age_ms bigint) RETURNS SETOF record LANGUAGE sql STABLE AS $$
  SELECT a.id, coalesce(l.daily_sat, default_sat), coalesce(l.pending_sat, 0), coalesce((extract(epoch FROM l.pending_at) * 1000)::bigint, 0),
         (extract(epoch FROM now()) * 1000)::bigint,
         (SELECT w.spent FROM auth.wallet_spent(a.id, '') w), auth.credit_spent(a.id),
         coalesce(((extract(epoch FROM now()) - extract(epoch FROM (SELECT max(s.l2_at) FROM auth.sessions s WHERE s.account = a.id))) * 1000)::bigint, -1)
    FROM auth.nostr_keys k JOIN auth.accounts a ON a.id = k.account AND a.kind = 'person' AND a.deleted_at IS NULL
    LEFT JOIN auth.wallet_limits l ON l.account = a.id
   WHERE k.pubkey = pk
$$;

-- The debit, as 0066 made it, now also counted for the account that holds the key (a person's).
CREATE OR REPLACE FUNCTION inference.finish(who text, model text, http integer, in_tokens integer, out_tokens integer, ms_taken integer, free boolean) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE
  cost bigint; rate bigint; m bigint := 0;
BEGIN
  cost := inference.finish(who, model, http, in_tokens, out_tokens, ms_taken);
  IF NOT free AND cost > 0 THEN
    SELECT usd_per_btc INTO rate FROM inference.btc_usd;
    -- msat = micros / 1e6 USD * 1e11 msat/BTC / usd_per_btc
    m := CASE WHEN coalesce(rate, 0) > 0 THEN (cost * 100000 + rate - 1) / rate ELSE 0 END;
    UPDATE inference.credit SET msat = msat - m, updated = now() WHERE pubkey = who;
    UPDATE inference.calls SET msat = m WHERE id = (SELECT max(id) FROM inference.calls WHERE pubkey = who);
    IF m > 0 THEN
      INSERT INTO auth.credit_spends AS c (account, hour, msat)
      SELECT k.account, date_trunc('hour', now()), m FROM auth.nostr_keys k JOIN auth.accounts a ON a.id = k.account AND a.kind = 'person'
       WHERE k.pubkey = who
      ON CONFLICT (account, hour) DO UPDATE SET msat = c.msat + EXCLUDED.msat;
    END IF;
  END IF;
  RETURN m;
END $$;

-- Rows older than 30 days say nothing the limit reads: the wallet's and the credit's.
CREATE OR REPLACE FUNCTION auth.wallet_spends_sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.wallet_spends WHERE at < now() - interval '30 days' RETURNING 1),
       c AS (DELETE FROM auth.credit_spends WHERE hour < now() - interval '30 days' RETURNING 1)
  SELECT ((SELECT count(*) FROM d) + (SELECT count(*) FROM c))::int
$$;

-- Deletion, merging and export (0149's registry).
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('auth.credit_spends', 'account', 'account', 'move', 'delete', true, '{}', 'service credit your model calls took, an hour at a time, counted against the daily limit');
