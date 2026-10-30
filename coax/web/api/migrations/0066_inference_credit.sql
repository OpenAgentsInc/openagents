-- Inference credit over Lightning (NIP-X402 openagents:spark-receiver:1;
-- inference.c answers POST /api/v1/inference/credit). A key buys prepaid
-- credit in millisatoshis with an invoice from our own Spark wallet
-- (coax/web/api/spark/), and /api/v1/inference spends it.
--
-- The binding between a payment and what it buys is ours, here: a top-up
-- row holds the invoice we asked our wallet for, its payment hash, the key
-- and amount it credits, and the request digest. The proof is our wallet's
-- record of that exact invoice paid, with a preimage that hashes to the
-- payment hash. The replay key (network:payment_hash) goes into
-- used_payments once, in the same transaction that adds the credit.

CREATE TABLE inference.credit (
  pubkey text PRIMARY KEY,
  msat bigint NOT NULL DEFAULT 0,             -- may go below 0 by one call's cost; then no more calls
  updated timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE inference.topups (
  payment_hash text PRIMARY KEY,
  invoice text NOT NULL UNIQUE,
  pubkey text NOT NULL,
  msat bigint NOT NULL,
  digest text NOT NULL,
  url text NOT NULL,
  pay_to text NOT NULL,
  expires timestamptz NOT NULL,
  created timestamptz NOT NULL DEFAULT now(),
  settled timestamptz
);
CREATE INDEX inference_topups_pubkey ON inference.topups (pubkey, created);

CREATE TABLE inference.used_payments (
  replay_key text PRIMARY KEY,                -- 'lnbtc:000000000019d6689c085ae165831e93:' || payment_hash
  pubkey text NOT NULL,
  msat bigint NOT NULL,
  at timestamptz NOT NULL DEFAULT now()
);

-- The BTC/USD rate, from our wallet (Breez's fiat rates), one row.
CREATE TABLE inference.btc_usd (
  one bool PRIMARY KEY DEFAULT true CHECK (one),
  usd_per_btc bigint NOT NULL CHECK (usd_per_btc > 0),
  at timestamptz NOT NULL
);

ALTER TABLE inference.calls ADD COLUMN msat bigint NOT NULL DEFAULT 0;   -- what a paying key's call took from its credit

CREATE FUNCTION inference.b64(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT translate(encode(convert_to(t, 'UTF8'), 'base64'), E'\n', '')
$$;

-- The network this profile takes: Bitcoin mainnet, and nothing else.
CREATE FUNCTION inference.network() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'lnbtc:000000000019d6689c085ae165831e93' $$;

-- What a top-up request asks for: {"status": 200, "pubkey", "msat", "sats",
-- "digest", "memo"}, or {"status": 400, "error"}. body is {"pubkey": 64
-- lowercase hex digits, "msat": a whole number of sats in msat, as a
-- decimal string or a number}, within [min_msat, max_msat].
CREATE FUNCTION inference.topup_terms(body text, url text, min_msat bigint, max_msat bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb; pk text; m numeric; d text;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' THEN RETURN '{"status":400,"error":"not JSON"}'; END IF;
  pk := b->>'pubkey';
  IF jsonb_typeof(b->'pubkey') IS DISTINCT FROM 'string' OR pk !~ '^[0-9a-f]{64}$' THEN RETURN '{"status":400,"error":"pubkey must be 64 lowercase hex digits"}'; END IF;
  IF jsonb_typeof(b->'msat') = 'string' AND b->>'msat' ~ '^[1-9][0-9]{0,15}$' THEN m := (b->>'msat')::numeric;
  ELSIF jsonb_typeof(b->'msat') = 'number' AND (b->>'msat') ~ '^[1-9][0-9]{0,15}$' THEN m := (b->>'msat')::numeric;
  ELSE RETURN '{"status":400,"error":"msat must be a positive whole number"}'; END IF;
  IF m % 1000 <> 0 THEN RETURN '{"status":400,"error":"msat must be whole sats"}'; END IF;
  IF m < min_msat OR m > max_msat THEN
    RETURN jsonb_build_object('status', 400, 'error', format('a top-up is %s to %s sats', min_msat / 1000, max_msat / 1000));
  END IF;
  d := encode(sha256(convert_to(format('openagents:spark-receiver:1|POST|%s|%s|%s', url, pk, m), 'UTF8')), 'hex');
  RETURN jsonb_build_object('status', 200, 'pubkey', pk, 'msat', m::bigint, 'sats', (m / 1000)::bigint, 'digest', d,
                            'memo', format('OpenAgents inference credit, %s sats. x402 %s', m / 1000, d));
END $$;

-- The x402 v2 challenge for a top-up row, as JSON.
CREATE FUNCTION inference.topup_required(t inference.topups, why text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'x402Version', 2, 'error', why,
    'resource', jsonb_build_object('url', t.url, 'description', 'OpenAgents inference credit', 'mimeType', 'application/json'),
    'accepts', jsonb_build_array(jsonb_build_object(
      'scheme', 'exact', 'network', inference.network(), 'amount', t.msat::text, 'asset', 'BTC', 'payTo', t.pay_to,
      'maxTimeoutSeconds', greatest(0, extract(epoch FROM t.expires - t.created)::bigint),
      'extra', jsonb_build_object('method', 'bolt11', 'paymentFlow', 'upfront', 'receiverProfile', 'openagents:spark-receiver:1',
                                  'invoice', t.invoice, 'paymentHash', t.payment_hash, 'requestDigest', t.digest))))
$$;

-- Keeps the invoice our wallet made for terms and gives the challenge:
-- {"status": 402, "header": base64 for PAYMENT-REQUIRED, "body": the same JSON},
-- or {"status": 502, "error"} when the wallet's answer doesn't fit.
CREATE FUNCTION inference.topup_issue(pk text, m bigint, d text, u text, inv text, hash text, pay_to text, expires_unix bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  t inference.topups; j jsonb;
BEGIN
  IF hash !~ '^[0-9a-f]{64}$' OR inv !~ '^lnbc[0-9a-z]+$' OR expires_unix <= extract(epoch FROM now()) THEN
    RETURN '{"status":502,"error":"the wallet''s invoice does not fit"}';
  END IF;
  INSERT INTO inference.topups (payment_hash, invoice, pubkey, msat, digest, url, pay_to, expires)
  VALUES (hash, inv, pk, m, d, u, coalesce(pay_to, ''), to_timestamp(expires_unix)) RETURNING * INTO t;
  j := inference.topup_required(t, 'payment required');
  RETURN jsonb_build_object('status', 402, 'header', inference.b64(j::text), 'body', j);
END $$;

-- Reads a retry's PAYMENT-SIGNATURE against the actual request: {"status":
-- 200, "invoice", "payment_hash", "preimage" (the payer's, or "")}, or why
-- not: 400 a bad payload, 402 terms that don't match one of our top-ups
-- (with the challenge again when there is one), 409 duplicate_settlement.
CREATE FUNCTION inference.topup_proof(body text, sig text, u text, min_msat bigint, max_msat bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  terms jsonb; p jsonb; a jsonb; hash text; pre text; t inference.topups;
BEGIN
  terms := inference.topup_terms(body, u, min_msat, max_msat);
  IF (terms->>'status')::int <> 200 THEN RETURN terms; END IF;
  BEGIN p := convert_from(decode(translate(sig, '-_', '+/') || repeat('=', (4 - length(sig) % 4) % 4), 'base64'), 'UTF8')::jsonb;
  EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"PAYMENT-SIGNATURE is not base64 JSON"}'; END;
  IF jsonb_typeof(p) IS DISTINCT FROM 'object' OR p->'x402Version' IS DISTINCT FROM '2'::jsonb THEN RETURN '{"status":400,"error":"x402Version must be 2"}'; END IF;
  a := p->'accepted';
  hash := p->'payload'->>'paymentHash';
  pre := coalesce(p->'payload'->>'preimage', '');
  IF jsonb_typeof(a) IS DISTINCT FROM 'object' OR hash IS NULL OR hash !~ '^[0-9a-f]{64}$' OR (pre <> '' AND pre !~ '^[0-9a-f]{64}$') THEN
    RETURN '{"status":400,"error":"the payload needs accepted and a payment hash"}';
  END IF;
  SELECT * INTO t FROM inference.topups WHERE payment_hash = hash;
  IF NOT FOUND THEN RETURN '{"status":402,"error":"invalid_payload: not an invoice we issued"}'; END IF;
  IF t.digest <> terms->>'digest' OR t.url <> u THEN
    RETURN jsonb_build_object('status', 402, 'error', 'invalid_payload: the request does not match the invoice');
  END IF;
  IF a->>'scheme' IS DISTINCT FROM 'exact' OR a->>'network' IS DISTINCT FROM inference.network() OR a->>'asset' IS DISTINCT FROM 'BTC'
     OR a->>'amount' IS DISTINCT FROM t.msat::text OR a->'extra'->>'invoice' IS DISTINCT FROM t.invoice
     OR a->'extra'->>'receiverProfile' IS DISTINCT FROM 'openagents:spark-receiver:1' THEN
    RETURN jsonb_build_object('status', 402, 'error', 'invalid_payload: accepted is not the terms we offered',
                              'header', inference.b64(inference.topup_required(t, 'invalid_payload')::text));
  END IF;
  IF t.settled IS NOT NULL THEN RETURN '{"status":409,"error":"duplicate_settlement"}'; END IF;
  RETURN jsonb_build_object('status', 200, 'invoice', t.invoice, 'payment_hash', t.payment_hash, 'preimage', pre,
                            'header', inference.b64(inference.topup_required(t, 'payment_pending')::text));
END $$;

-- Settles a top-up from our wallet's record of its invoice, in one
-- transaction: {"status": 200, "credit_msat", "header": base64 for
-- PAYMENT-RESPONSE}, or 402 (not paid, wrong amount or preimage), 409
-- duplicate_settlement. state, amount and preimage are the wallet's;
-- payer_preimage is the payer's ("" when its wallet gave none).
CREATE FUNCTION inference.topup_settle(hash text, state text, amount_msat bigint, preimage text, payer_preimage text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  t inference.topups; c bigint; rk text;
BEGIN
  SELECT * INTO t FROM inference.topups WHERE payment_hash = hash FOR UPDATE;
  IF NOT FOUND THEN RETURN '{"status":402,"error":"invalid_payload: not an invoice we issued"}'; END IF;
  IF state IS DISTINCT FROM 'completed' THEN RETURN '{"status":402,"error":"payment_pending"}'; END IF;
  IF amount_msat IS NULL OR amount_msat < t.msat THEN RETURN '{"status":402,"error":"invalid_payment: less than the invoice"}'; END IF;
  IF preimage IS NULL OR preimage !~ '^[0-9a-f]{64}$' OR encode(sha256(decode(preimage, 'hex')), 'hex') <> t.payment_hash THEN
    RETURN '{"status":402,"error":"invalid_payment: the preimage does not hash to the payment hash"}';
  END IF;
  IF coalesce(payer_preimage, '') <> '' AND payer_preimage <> preimage THEN
    RETURN '{"status":402,"error":"invalid_payment: the payer''s preimage is not the invoice''s"}';
  END IF;
  rk := inference.network() || ':' || t.payment_hash;
  INSERT INTO inference.used_payments (replay_key, pubkey, msat) VALUES (rk, t.pubkey, t.msat) ON CONFLICT DO NOTHING;
  IF NOT FOUND THEN RETURN '{"status":409,"error":"duplicate_settlement"}'; END IF;
  UPDATE inference.topups SET settled = now() WHERE payment_hash = t.payment_hash;
  INSERT INTO inference.credit AS k (pubkey, msat) VALUES (t.pubkey, t.msat)
  ON CONFLICT (pubkey) DO UPDATE SET msat = k.msat + EXCLUDED.msat, updated = now() RETURNING msat INTO c;
  RETURN jsonb_build_object('status', 200, 'credit_msat', c, 'pubkey', t.pubkey, 'msat', t.msat,
    'header', inference.b64(jsonb_build_object('success', true, 'transaction', t.payment_hash, 'network', inference.network(),
                                               'payer', t.pubkey, 'amount', t.msat::text)::text));
END $$;

-- The rate: kept when the wallet gives one; its age in seconds (null: none).
CREATE FUNCTION inference.rate_set(usd bigint) RETURNS void LANGUAGE sql AS $$
  INSERT INTO inference.btc_usd (usd_per_btc, at) VALUES (usd, now())
  ON CONFLICT (one) DO UPDATE SET usd_per_btc = EXCLUDED.usd_per_btc, at = EXCLUDED.at
$$;
CREATE FUNCTION inference.rate_age() RETURNS double precision LANGUAGE sql STABLE AS $$
  SELECT extract(epoch FROM now() - at) FROM inference.btc_usd
$$;

-- Admission with credit: inference.request's answer, after it, for a key
-- that pays (not an admin or on the list): 402 with no credit left, 503
-- with no rate younger than an hour. free keys are only counted.
CREATE FUNCTION inference.request(who text, body text, cap_n int, cap_usd_micros bigint, free bool) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  c bigint; age double precision; r jsonb;
BEGIN
  IF NOT free THEN
    SELECT msat INTO c FROM inference.credit WHERE pubkey = who;
    IF coalesce(c, 0) <= 0 THEN
      RETURN jsonb_build_object('status', 402, 'error', 'add credit: POST /api/v1/inference/credit', 'credit', '/api/v1/inference/credit',
                                'credit_msat', coalesce(c, 0));
    END IF;
    age := inference.rate_age();
    IF age IS NULL OR age > 3600 THEN RETURN '{"status":503,"error":"no bitcoin price to charge at; try again soon"}'; END IF;
  END IF;
  r := inference.request(who, body, cap_n, cap_usd_micros);
  RETURN r || jsonb_build_object('free', free);
END $$;

-- Keeps a call (inference.finish) and, for a paying key, takes its cost
-- from its credit in msat at the kept rate, rounded up: the msat taken.
CREATE FUNCTION inference.finish(who text, model text, http int, in_tokens int, out_tokens int, ms_taken int, free bool) RETURNS bigint LANGUAGE plpgsql AS $$
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
  END IF;
  RETURN m;
END $$;
