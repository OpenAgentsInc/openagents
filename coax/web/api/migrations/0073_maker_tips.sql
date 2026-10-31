-- Tips for a realm's maker (ev-maker-tips, docs/verse/makers.md). A player asks for an invoice
-- from our Spark wallet (coax/web/api/spark/) for a claimed realm; once our wallet's record shows it
-- paid, with a preimage that hashes to its payment hash, the tip is kept once (the replay ledger,
-- inference.used_payments, the same one inference credit uses) and a payout to the maker's
-- Lightning address is queued in the same transaction. makers.c sends it from our wallet: the
-- maker gets the tip less the routing fee (OpenAgents keeps nothing else), and during launch the
-- first player tip to each maker carries a first-paid bonus out of our own sats.
--
-- An admin's gift (the owner paying a maker from our wallet) is a tip with no invoice.
-- Bitcoin only: Lightning on mainnet. Additive: nothing here changes another table's rows
-- except the new tips' replay keys in inference.used_payments.

CREATE TABLE makers.tips (
  payment_hash text PRIMARY KEY,               -- our invoice's; a gift's is 'gift:' and 32 random hex digits
  invoice text UNIQUE,                         -- our wallet's invoice the player pays; NULL for a gift
  realm text NOT NULL,
  maker text NOT NULL,                         -- the maker's pubkey when the tip was asked
  lightning text NOT NULL,                     -- their address then
  sats bigint NOT NULL CHECK (sats > 0),
  team boolean NOT NULL DEFAULT false,         -- our team asked (oa_team, an admin): real money, left out of usage
  gift boolean NOT NULL DEFAULT false,
  expires timestamptz NOT NULL,
  created timestamptz NOT NULL DEFAULT now(),
  paid timestamptz
);
CREATE INDEX makers_tips_realm ON makers.tips (realm, created);

CREATE TABLE makers.payouts (
  tip text PRIMARY KEY REFERENCES makers.tips (payment_hash),   -- one payout a tip, ever
  realm text NOT NULL,
  maker text NOT NULL,
  lightning text NOT NULL,
  sats bigint NOT NULL CHECK (sats > 0),       -- what leaves our wallet: the tip and the bonus; the routing fee comes out of it
  bonus_sats bigint NOT NULL DEFAULT 0,
  state text NOT NULL DEFAULT 'queued' CHECK (state IN ('queued', 'quoted', 'sending', 'sent', 'failed')),
  invoice text UNIQUE,                         -- the maker's invoice, from their address: the one this payout pays
  invoice_hash text,
  invoice_expires timestamptz,
  amount_sats bigint,                          -- what that invoice is for (sats less the fee)
  fee_sats bigint,                             -- the routing fee
  attempts int NOT NULL DEFAULT 0,
  next_try timestamptz NOT NULL DEFAULT now(),
  lease timestamptz,                           -- a runner holds it until then
  error text,
  sent timestamptz,
  created timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX makers_payouts_due ON makers.payouts (next_try) WHERE state <> 'sent';

-- What a tip asks: {"status": 200, "maker", "lightning", "sats", "memo"}, or 400 / 404.
CREATE FUNCTION makers.tip_terms(r text, body text, min_sats bigint, max_sats bigint) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  b jsonb; m makers.realms; n numeric;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  SELECT * INTO m FROM makers.realms WHERE realm = r;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"nobody has claimed this realm yet"}'; END IF;
  IF jsonb_typeof(b->'sats') IS DISTINCT FROM 'number' OR (b->>'sats') !~ '^[1-9][0-9]{0,8}$' THEN
    RETURN '{"status":400,"error":"sats must be a whole number"}';
  END IF;
  n := (b->>'sats')::numeric;
  IF n < min_sats OR n > max_sats THEN RETURN jsonb_build_object('status', 400, 'error', format('a tip is %s to %s sats', min_sats, max_sats)); END IF;
  RETURN jsonb_build_object('status', 200, 'maker', m.pubkey, 'lightning', m.lightning, 'sats', n::bigint,
                            'memo', format('Tip for %s on OpenAgents', m.name));
END $$;

-- Keeps the invoice our wallet made for a tip: {"status": 200, "invoice", "payment_hash", "sats",
-- "expires"} or 502 when the wallet's answer doesn't fit.
CREATE FUNCTION makers.tip_issue(r text, n bigint, inv text, hash text, amount_msat bigint, expires_unix bigint, is_team boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m makers.realms;
BEGIN
  SELECT * INTO m FROM makers.realms WHERE realm = r;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"nobody has claimed this realm yet"}'; END IF;
  IF hash !~ '^[0-9a-f]{64}$' OR inv !~ '^lnbc[0-9a-z]+$' OR amount_msat <> n * 1000 OR expires_unix <= extract(epoch FROM now()) THEN
    RETURN '{"status":502,"error":"our wallet''s invoice does not fit"}';
  END IF;
  INSERT INTO makers.tips (payment_hash, invoice, realm, maker, lightning, sats, team, expires)
  VALUES (hash, inv, r, m.pubkey, m.lightning, n, coalesce(is_team, false), to_timestamp(expires_unix));
  RETURN jsonb_build_object('status', 200, 'invoice', inv, 'payment_hash', hash, 'sats', n, 'expires', expires_unix);
END $$;

-- Where a tip stands, for the page: {"status": 200, "state": "open"|"paid"|"expired", "sats",
-- "payout": "queued"|...|"sent"|"" , "to": the maker's name}, or 404.
CREATE FUNCTION makers.tip_state(r text, hash text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((
    SELECT jsonb_build_object('status', 200, 'sats', t.sats, 'invoice', t.invoice,
             'state', CASE WHEN t.paid IS NOT NULL THEN 'paid' WHEN t.expires < now() THEN 'expired' ELSE 'open' END,
             'payout', coalesce(p.state, ''), 'to', coalesce(m.name, ''))
    FROM makers.tips t LEFT JOIN makers.payouts p ON p.tip = t.payment_hash LEFT JOIN makers.realms m ON m.realm = t.realm
    WHERE t.payment_hash = hash AND t.realm = r AND NOT t.gift), '{"status":404,"error":"no such tip"}'::jsonb)
$$;

-- Queues a paid tip's payout. The first paid player tip to a maker (not a gift) carries bonus sats.
CREATE FUNCTION makers.queue_payout(t makers.tips, bonus bigint) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  b bigint := 0;
BEGIN
  IF NOT t.gift AND coalesce(bonus, 0) > 0 AND NOT EXISTS (
       SELECT 1 FROM makers.payouts p WHERE p.maker = t.maker AND p.bonus_sats > 0) THEN
    b := bonus;
  END IF;
  INSERT INTO makers.payouts (tip, realm, maker, lightning, sats, bonus_sats) VALUES (t.payment_hash, t.realm, t.maker, t.lightning, t.sats + b, b);
  RETURN b;
END $$;

-- Settles a tip from our wallet's record of its invoice, in one transaction: the replay key into
-- inference.used_payments once, the tip paid, its payout queued. {"status": 200, "state": "paid",
-- "bonus_sats"}, or 402 (not paid yet, the wrong amount or preimage), 409 (settled before).
CREATE FUNCTION makers.tip_settle(hash text, state text, amount_msat bigint, preimage text, bonus bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  t makers.tips; rk text; b bigint;
BEGIN
  SELECT * INTO t FROM makers.tips WHERE payment_hash = hash AND NOT gift FOR UPDATE;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"no such tip"}'; END IF;
  IF t.paid IS NOT NULL THEN RETURN '{"status":200,"state":"paid","again":true}'; END IF;
  IF state IS DISTINCT FROM 'completed' THEN RETURN jsonb_build_object('status', 402, 'error', 'not paid yet', 'state', coalesce(state, 'none')); END IF;
  IF amount_msat IS DISTINCT FROM t.sats * 1000 THEN RETURN '{"status":402,"error":"paid the wrong amount"}'; END IF;
  IF preimage !~ '^[0-9a-f]{64}$' OR encode(sha256(decode(preimage, 'hex')), 'hex') <> hash THEN
    RETURN '{"status":402,"error":"the preimage does not match"}';
  END IF;
  rk := inference.network() || ':' || hash;
  INSERT INTO inference.used_payments (replay_key, pubkey, msat) VALUES (rk, t.maker, t.sats * 1000) ON CONFLICT DO NOTHING;
  IF NOT FOUND THEN RETURN '{"status":409,"error":"duplicate_settlement"}'; END IF;
  UPDATE makers.tips SET paid = now() WHERE payment_hash = hash RETURNING * INTO t;
  b := makers.queue_payout(t, bonus);
  RETURN jsonb_build_object('status', 200, 'state', 'paid', 'bonus_sats', b);
END $$;

-- An admin's gift from our wallet to a realm's maker: a tip with no invoice, paid now, its payout
-- queued. {"status": 200, "tip", "sats"}, or 400 / 404.
CREATE FUNCTION makers.gift(r text, n bigint, max_sats bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m makers.realms; t makers.tips;
BEGIN
  SELECT * INTO m FROM makers.realms WHERE realm = r;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"nobody has claimed this realm yet"}'; END IF;
  IF n IS NULL OR n < 1 OR n > max_sats THEN RETURN jsonb_build_object('status', 400, 'error', format('a gift is 1 to %s sats', max_sats)); END IF;
  INSERT INTO makers.tips (payment_hash, realm, maker, lightning, sats, team, gift, expires, paid)
  VALUES ('gift:' || md5(random()::text || clock_timestamp()::text), r, m.pubkey, m.lightning, n, true, true, now(), now()) RETURNING * INTO t;
  PERFORM makers.queue_payout(t, 0);
  RETURN jsonb_build_object('status', 200, 'tip', t.payment_hash, 'sats', n);
END $$;

-- The next payout due, leased to this runner for lease_s seconds; NULL when none is due or the
-- day's payouts (sats sent and being sent since Chicago's midnight) would pass day_cap.
CREATE FUNCTION makers.payout_next(lease_s int, day_cap bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  p makers.payouts; spent bigint;
BEGIN
  SELECT * INTO p FROM makers.payouts
   WHERE state <> 'sent' AND next_try <= now() AND (lease IS NULL OR lease < now()) AND attempts < 30
   ORDER BY next_try, created FOR UPDATE SKIP LOCKED LIMIT 1;
  IF NOT FOUND THEN RETURN NULL; END IF;
  SELECT coalesce(sum(sats), 0) INTO spent FROM makers.payouts
   WHERE tip <> p.tip AND (state IN ('sending', 'quoted') OR (state = 'sent' AND sent >= date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago'));
  IF spent + p.sats > day_cap THEN
    UPDATE makers.payouts SET next_try = now() + interval '1 hour', error = 'the day''s payouts are used up' WHERE tip = p.tip;
    RETURN NULL;
  END IF;
  UPDATE makers.payouts SET lease = now() + make_interval(secs => lease_s), attempts = attempts + 1 WHERE tip = p.tip RETURNING * INTO p;
  RETURN jsonb_build_object('tip', p.tip, 'lightning', p.lightning, 'sats', p.sats, 'state', p.state, 'invoice', coalesce(p.invoice, ''),
                            'expired', p.invoice_expires IS NOT NULL AND p.invoice_expires < now() - interval '10 minutes',
                            'realm', p.realm, 'attempts', p.attempts);
END $$;

-- The maker's invoice for a payout, before it is sent: the payout pays this one and no other
-- until our wallet says it failed or it expired unpaid.
CREATE FUNCTION makers.payout_quoted(t text, inv text, hash text, amount bigint, fee bigint, expires_unix bigint) RETURNS boolean LANGUAGE plpgsql AS $$
BEGIN
  IF inv !~ '^lnbc[0-9a-z]+$' OR hash !~ '^[0-9a-f]{64}$' OR amount < 1 OR fee < 0 THEN RETURN false; END IF;
  UPDATE makers.payouts SET state = 'quoted', invoice = inv, invoice_hash = hash, amount_sats = amount, fee_sats = fee,
    invoice_expires = to_timestamp(expires_unix), error = NULL
   WHERE tip = t AND state <> 'sent' AND amount + fee <= sats;
  RETURN FOUND;
END $$;

-- What became of a payout: 'sent' (done, never again), 'pending' (ask again in a minute),
-- 'failed' (again later, backing off; requote: forget the invoice, the wallet said it failed or it
-- expired unpaid).
CREATE FUNCTION makers.payout_result(t text, outcome text, fee bigint, why text, requote boolean) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF outcome = 'sent' THEN
    UPDATE makers.payouts SET state = 'sent', sent = now(), lease = NULL, error = NULL, fee_sats = coalesce(fee, fee_sats) WHERE tip = t AND state <> 'sent';
  ELSIF outcome = 'pending' THEN
    UPDATE makers.payouts SET state = 'sending', lease = NULL, next_try = now() + interval '1 minute' WHERE tip = t AND state <> 'sent';
  ELSE
    UPDATE makers.payouts SET state = 'failed', lease = NULL, error = left(coalesce(why, 'failed'), 200),
      next_try = now() + least(interval '6 hours', make_interval(mins => (2 ^ least(attempts, 9))::int)),
      invoice = CASE WHEN requote THEN NULL ELSE invoice END, invoice_hash = CASE WHEN requote THEN NULL ELSE invoice_hash END,
      invoice_expires = CASE WHEN requote THEN NULL ELSE invoice_expires END
     WHERE tip = t AND state <> 'sent';
  END IF;
END $$;

-- Payouts now, for an admin: a line each.
CREATE FUNCTION makers.payout_lines() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(format('%s|%s|%s|%s|%s|%s|%s', p.realm, p.state, p.sats, p.bonus_sats, coalesce(p.fee_sats::text, ''),
                                    p.attempts, coalesce(p.error, '')), E'\n' ORDER BY p.created), '')
  FROM makers.payouts p
$$;

-- Our wallet can't pay a first tip's bonus yet (its balance is short of it): the tip goes now,
-- and the bonus becomes a payout of its own that waits until we fund our wallet. Returns the
-- payout's new sats, or NULL when there was no bonus to set aside.
CREATE FUNCTION makers.defer_bonus(t text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  p makers.payouts; b text;
BEGIN
  SELECT * INTO p FROM makers.payouts WHERE tip = t AND bonus_sats > 0 AND sats > bonus_sats AND state <> 'sent' AND invoice IS NULL FOR UPDATE;
  IF NOT FOUND THEN RETURN NULL; END IF;
  b := 'bonus:' || md5(t);
  INSERT INTO makers.tips (payment_hash, realm, maker, lightning, sats, team, gift, expires, paid)
  VALUES (b, p.realm, p.maker, p.lightning, p.bonus_sats, true, true, now(), now());
  INSERT INTO makers.payouts (tip, realm, maker, lightning, sats, bonus_sats, next_try)
  VALUES (b, p.realm, p.maker, p.lightning, p.bonus_sats, p.bonus_sats, now() + interval '10 minutes');
  UPDATE makers.payouts SET sats = sats - bonus_sats, bonus_sats = 0 WHERE tip = t;
  RETURN p.sats - p.bonus_sats;
END $$;
