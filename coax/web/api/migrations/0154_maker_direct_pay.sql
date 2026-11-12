-- Tips and upgrade purchases go from the player's wallet straight to the maker's own Lightning
-- Address (owner decision D6, docs/auth/lightning-addresses.md "Paying makers' own addresses",
-- docs/verse/makers.md, #13210). They never enter our wallet. Our server asks the maker's address
-- for the invoice (coax/web/api/lnaddr.c, coax/lib/lnaddr/lnurl.coax check it) and the player's
-- wallet pays it. A payment counts only when confirmed: the maker's service says settled with a
-- preimage (LUD-21), or the player's wallet reports the preimage; either way SHA-256 of the
-- preimage is the invoice's payment hash. Every rule is makerpay-policy.coax's; makers.c gathers
-- the facts and these functions keep the rows, each with a second wall of its own.
--
-- What changes:
--   makers.tips      a direct row keeps the maker's invoice, the LUD-21 URL (never shown), how it
--                    was confirmed and the preimage that proves it
--   makers.payouts   our wallet sends only our own money now: a maker's first-tip bonus (its own
--                    payment, source 'bonus') and an admin's gift ('gift'). A direct payment can
--                    never have a payout of the player's money (the trigger below).
--   credit           an upgrade can no longer be bought with service credit (makers.buy_credit).
--
-- Nothing is dropped and no row is rewritten. Tips and purchases paid into our wallet before this
-- (direct = false) keep their rows; a payout of theirs not yet sent is still sent by the same
-- runner, and an invoice of ours still open is still settled the old way until it expires.

ALTER TABLE makers.tips ADD COLUMN direct boolean NOT NULL DEFAULT false;   -- the invoice is the maker's own
ALTER TABLE makers.tips ADD COLUMN verify_url text CHECK (length(verify_url) <= 1024);   -- the maker's service's LUD-21 URL
ALTER TABLE makers.tips ADD COLUMN confirmed_by text CHECK (confirmed_by IN ('verify', 'wallet'));
ALTER TABLE makers.tips ADD COLUMN preimage text CHECK (preimage ~ '^[0-9a-f]{64}$');
-- A direct payment is paid only with its proof.
ALTER TABLE makers.tips ADD CONSTRAINT tips_direct_proof
  CHECK (NOT direct OR paid IS NULL OR (confirmed_by IS NOT NULL AND preimage IS NOT NULL AND encode(sha256(decode(preimage, 'hex')), 'hex') = payment_hash));

ALTER TABLE makers.payouts DROP CONSTRAINT payouts_source_check;
ALTER TABLE makers.payouts ADD CONSTRAINT payouts_source_check CHECK (source IN ('tip', 'upgrade', 'bonus', 'gift'));

-- No payout of a player's money for a direct payment: it was never with us.
CREATE FUNCTION makers.payout_wall() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.source IN ('tip', 'upgrade') AND (SELECT t.direct FROM makers.tips t WHERE t.payment_hash = NEW.tip) THEN
    RAISE EXCEPTION 'makers: a payment made straight to the maker has no payout from our wallet (docs/verse/makers.md)' USING ERRCODE = 'check_violation';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER payout_wall BEFORE INSERT OR UPDATE OF source, tip ON makers.payouts FOR EACH ROW EXECUTE FUNCTION makers.payout_wall();

-- As 0073's, and an admin's gift is named as one.
CREATE OR REPLACE FUNCTION makers.queue_payout(t makers.tips, bonus bigint) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE
  b bigint := 0;
BEGIN
  IF NOT t.gift AND coalesce(bonus, 0) > 0 AND NOT EXISTS (
       SELECT 1 FROM makers.payouts p WHERE p.maker = t.maker AND p.bonus_sats > 0) THEN
    b := bonus;
  END IF;
  INSERT INTO makers.payouts (tip, realm, maker, lightning, sats, bonus_sats, source)
  VALUES (t.payment_hash, t.realm, t.maker, t.lightning, t.sats + b, b, CASE WHEN t.gift THEN 'gift' ELSE 'tip' END);
  RETURN b;
END $$;

-- As 0073's, with the payout's source and whether its payment was direct (makerpay-policy.coax
-- mp_payout decides whether our wallet may send it).
CREATE OR REPLACE FUNCTION makers.payout_next(lease_s int, day_cap bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
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
                            'realm', p.realm, 'attempts', p.attempts, 'source', p.source,
                            'direct', coalesce((SELECT t.direct FROM makers.tips t WHERE t.payment_hash = p.tip), false));
END $$;

-- As 0077's, with the listing's Lightning Address (never sent to the player: makers.c asks it for the invoice).
CREATE OR REPLACE FUNCTION makers.buy_terms(l bigint, pl text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  x makers.listings;
BEGIN
  IF pl IS NULL THEN RETURN '{"status":403,"error":"no player"}'; END IF;
  SELECT * INTO x FROM makers.listings WHERE id = l AND state = 'approved';
  IF NOT FOUND THEN RETURN '{"status":404,"error":"this fairy isn''t listed"}'; END IF;
  IF makers.owns(pl, l) THEN RETURN '{"status":409,"error":"it is yours already"}'; END IF;
  RETURN jsonb_build_object('status', 200, 'sats', x.sats, 'lightning', x.lightning,
                            'memo', format('%s, a fairy by %s, on OpenAgents', x.name, x.maker_name));
END $$;

-- Service credit pays only for our own services (decision D4): never an upgrade. The credit is not touched.
CREATE OR REPLACE FUNCTION makers.buy_credit(l bigint, pl text, pk text, is_team boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT '{"status":400,"error":"Upgrades are paid in Bitcoin, straight to the maker; service credit pays only for our own services"}'::jsonb
$$;

-- Keeps the maker's own invoice for a tip (l NULL: realm r's maker) or a purchase (listing l, player
-- pl), after makers.c had it checked. addr is the address the invoice came from: when the maker's
-- address is no longer that one, nothing is kept (a maker who changes address mid-payment gets a
-- new invoice). {"status": 200}, or 404 (no maker, no listing, no player), 409 (the address or the
-- price changed), 502 (the invoice doesn't fit, or its payment hash was seen before).
CREATE FUNCTION makers.direct_keep(r text, l bigint, pl text, addr text, n bigint, inv text, hash text, verify text, expires_unix bigint, is_team boolean)
RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m makers.realms; x makers.listings;
BEGIN
  IF hash !~ '^[0-9a-f]{64}$' OR inv !~ '^ln[a-z0-9]+$' OR n IS NULL OR n < 1 OR expires_unix IS NULL OR expires_unix <= extract(epoch FROM now()) THEN
    RETURN '{"status":502}';
  END IF;
  IF EXISTS (SELECT 1 FROM makers.tips WHERE payment_hash = hash) THEN RETURN '{"status":502}'; END IF;
  IF l IS NULL THEN
    SELECT * INTO m FROM makers.realms WHERE realm = r FOR SHARE;
    IF NOT FOUND THEN RETURN '{"status":404}'; END IF;
    IF m.lightning IS DISTINCT FROM addr THEN RETURN '{"status":409}'; END IF;
    INSERT INTO makers.tips (payment_hash, invoice, realm, maker, lightning, sats, team, expires, direct, verify_url)
    VALUES (hash, inv, r, m.pubkey, m.lightning, n, coalesce(is_team, false), to_timestamp(expires_unix), true, nullif(verify, ''));
  ELSE
    SELECT * INTO x FROM makers.listings WHERE id = l AND state = 'approved' FOR SHARE;
    IF NOT FOUND OR pl IS NULL THEN RETURN '{"status":404}'; END IF;
    IF x.lightning IS DISTINCT FROM addr OR x.sats <> n THEN RETURN '{"status":409}'; END IF;
    INSERT INTO makers.tips (payment_hash, invoice, realm, maker, lightning, sats, team, expires, direct, verify_url, listing, buyer, paid_with)
    VALUES (hash, inv, 'fairy-forest', x.maker, x.lightning, n, coalesce(is_team, false), to_timestamp(expires_unix), true, nullif(verify, ''), l, pl, 'sats');
  END IF;
  RETURN '{"status":200}';
EXCEPTION WHEN unique_violation THEN
  RETURN '{"status":502}';
END $$;

-- The facts of a direct payment, for makers.c and makerpay-policy.coax: a tip of realm r (l NULL) or
-- player pl's purchase of listing l. {"found": false} for none, and for a payment made to our wallet
-- before D6 (direct false). The LUD-21 URL is read here and never sent to a player.
CREATE FUNCTION makers.direct_facts(hash text, r text, l bigint, pl text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((
    SELECT jsonb_build_object('found', true, 'direct', t.direct, 'paid', t.paid IS NOT NULL, 'expired', t.expires < now(),
             'verify', coalesce(t.verify_url, ''), 'sats', t.sats, 'maker', t.maker, 'purchase', t.listing IS NOT NULL,
             'had_bonus', EXISTS (SELECT 1 FROM makers.payouts p WHERE p.maker = t.maker AND p.bonus_sats > 0))
    FROM makers.tips t
    WHERE t.payment_hash = hash AND NOT t.gift
      AND ((l IS NULL AND t.listing IS NULL AND t.realm = r) OR (l IS NOT NULL AND t.listing = l AND t.buyer = pl))),
    '{"found":false}'::jsonb)
$$;

-- Counts a direct payment, once, with its proof: by 'verify' (the maker's service's LUD-21 answer)
-- or 'wallet' (the player's wallet's preimage). In one transaction: the replay key into
-- inference.used_payments (the ledger every payment hash goes through once), the row paid with its
-- preimage, a purchase worn, and for a tip the maker's first-tip bonus (bonus sats, 0 for none)
-- queued as a payment of its own from our wallet. {"status": 200, "state": "paid", "bonus_sats"},
-- 200 with "again" (counted before: nothing changes), 402 (the preimage isn't this invoice's),
-- 404, 409 (the hash was settled elsewhere).
CREATE FUNCTION makers.direct_confirm(hash text, how text, pre text, bonus bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  t makers.tips; rk text; b bigint := 0; bt text;
BEGIN
  SELECT * INTO t FROM makers.tips WHERE payment_hash = hash AND direct AND NOT gift FOR UPDATE;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"No such payment"}'; END IF;
  IF t.paid IS NOT NULL THEN RETURN '{"status":200,"state":"paid","again":true,"bonus_sats":0}'; END IF;
  IF how IS NULL OR how NOT IN ('verify', 'wallet') OR pre IS NULL OR pre !~ '^[0-9a-f]{64}$'
     OR encode(sha256(decode(pre, 'hex')), 'hex') <> hash THEN
    RETURN '{"status":402,"error":"That preimage isn''t this invoice''s"}';
  END IF;
  rk := inference.network() || ':' || hash;
  INSERT INTO inference.used_payments (replay_key, pubkey, msat) VALUES (rk, t.maker, t.sats * 1000) ON CONFLICT DO NOTHING;
  IF NOT FOUND THEN RETURN '{"status":409,"error":"duplicate_settlement"}'; END IF;
  UPDATE makers.tips SET paid = now(), confirmed_by = how, preimage = pre WHERE payment_hash = hash;
  IF t.listing IS NOT NULL THEN
    INSERT INTO makers.wearing (player, listing) VALUES (t.buyer, t.listing) ON CONFLICT (player) DO UPDATE SET listing = excluded.listing, since = now();
  ELSIF coalesce(bonus, 0) > 0 THEN
    PERFORM pg_advisory_xact_lock(hashtext('makers.bonus:' || t.maker));   -- one bonus a maker, whatever arrives at once
    IF NOT EXISTS (SELECT 1 FROM makers.payouts p WHERE p.maker = t.maker AND p.bonus_sats > 0) THEN
      b := bonus; bt := 'bonus:' || md5(hash);
      INSERT INTO makers.tips (payment_hash, realm, maker, lightning, sats, team, gift, expires, paid)
      VALUES (bt, t.realm, t.maker, t.lightning, b, true, true, now(), now());
      INSERT INTO makers.payouts (tip, realm, maker, lightning, sats, bonus_sats, source) VALUES (bt, t.realm, t.maker, t.lightning, b, b, 'bonus');
    END IF;
  END IF;
  RETURN jsonb_build_object('status', 200, 'state', 'paid', 'bonus_sats', b);
END $$;

-- What the page is told about a direct payment (never the LUD-21 URL): {"sats", "to", "confirm":
-- "verify" (the maker's service can say it arrived) or "wallet" (only the paying wallet can),
-- "bonus": the first-tip bonus's payout state or "", and for a purchase "id" and "look"}.
CREATE FUNCTION makers.direct_shown(hash text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('sats', t.sats, 'to', coalesce(CASE WHEN t.listing IS NULL THEN m.name ELSE x.maker_name END, ''),
           'confirm', CASE WHEN t.verify_url IS NOT NULL THEN 'verify' ELSE 'wallet' END,
           'bonus', coalesce((SELECT p.state FROM makers.payouts p WHERE p.tip = 'bonus:' || md5(t.payment_hash)), ''))
         || CASE WHEN t.listing IS NOT NULL THEN jsonb_build_object('id', t.listing, 'look', x.look) ELSE '{}'::jsonb END
  FROM makers.tips t LEFT JOIN makers.realms m ON m.realm = t.realm LEFT JOIN makers.listings x ON x.id = t.listing
  WHERE t.payment_hash = hash AND t.direct
$$;

-- Payouts now, for an admin: a line each, with what each is (tip and upgrade: paid into our wallet
-- before D6; bonus and gift: our own).
CREATE OR REPLACE FUNCTION makers.payout_lines() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(format('%s|%s|%s|%s|%s|%s|%s|%s', p.realm, p.state, p.sats, p.bonus_sats, coalesce(p.fee_sats::text, ''),
                                    p.attempts, coalesce(p.error, ''), p.source), E'\n' ORDER BY p.created), '')
  FROM makers.payouts p
$$;
