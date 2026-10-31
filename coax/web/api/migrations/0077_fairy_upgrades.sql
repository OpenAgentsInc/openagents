-- Listed upgrades (ev-asset-upgrades, docs/verse/makers.md "Fairy upgrades"): someone makes a better
-- fairy and lists it; an admin approves the listing; in Fairy Forest UPGRADE shows the top three, and
-- one tap buys one, with sats (our Spark wallet's invoice, settled the way a tip is) or with the
-- buyer's inference credit (the existing credit ledger, inference.credit, in msat). The maker is paid
-- the price through the same payout queue as tips (makers.payouts, makers.c run_payouts), less the
-- routing fee. The bought fairy is worn by the buyer's player (the X-Verse-Player key), and the room
-- (verse.room_seen) hands its look to everyone else in the forest.
--
-- A fairy listing is a look Fairy Forest draws: a look code, the nine parts of the maker
-- (verse/fairy-forest/life.coax FFL_CHOICES, a digit each) and then, optionally, seven colours of
-- six hex digits (hair, bodice, skirt, gloves and under-skirt, the three wing pairs) that no choice
-- in the maker gives. Bitcoin only. Additive: new tables and columns; makers.tips and makers.payouts
-- gain nullable columns (a source defaulting to 'tip'), and verse.room_seen answers one more field.

CREATE TABLE makers.listings (
  id bigserial PRIMARY KEY,
  kind text NOT NULL DEFAULT 'fairy' CHECK (kind IN ('fairy')),
  name text NOT NULL,                          -- the listing's, 1 to 40 characters (makers.name_of)
  look text NOT NULL,                          -- a look code (makers.look_of)
  sats bigint NOT NULL CHECK (sats > 0),       -- its price
  maker text NOT NULL,                         -- the maker's pubkey
  maker_name text NOT NULL,
  lightning text NOT NULL,                     -- where the maker's share goes (LUD-16)
  state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'approved', 'rejected')),
  created timestamptz NOT NULL DEFAULT now(),
  decided timestamptz,
  decided_by text                              -- the admin's pubkey
);
CREATE INDEX makers_listings_state ON makers.listings (kind, state, created);

-- A purchase is a tip with a listing: its payment hash ours (sats) or 'credit:' and 32 hex (credit).
ALTER TABLE makers.tips ADD COLUMN listing bigint REFERENCES makers.listings (id);
ALTER TABLE makers.tips ADD COLUMN buyer text;          -- the verse player who bought it
ALTER TABLE makers.tips ADD COLUMN buyer_key text;      -- their logged-in pubkey, when they paid with credit
ALTER TABLE makers.tips ADD COLUMN paid_with text CHECK (paid_with IN ('sats', 'credit'));
CREATE INDEX makers_tips_listing ON makers.tips (listing, buyer) WHERE listing IS NOT NULL;
ALTER TABLE makers.payouts ADD COLUMN source text NOT NULL DEFAULT 'tip' CHECK (source IN ('tip', 'upgrade'));

-- What a player wears: one bought listing, or no row (the fairy they made).
CREATE TABLE makers.wearing (
  player text PRIMARY KEY,
  listing bigint NOT NULL REFERENCES makers.listings (id),
  since timestamptz NOT NULL DEFAULT now()
);

-- A look code: nine part digits within the maker's choices, and optionally seven colours. Lowercased.
CREATE FUNCTION makers.look_of(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN l ~ '^[0-2][0-5][0-6][0-2][0-4][0-5][0-2][0-4][0-2]([0-9a-f]{42})?$' THEN l END
  FROM (SELECT lower(btrim(coalesce(t, ''))) AS l) x
$$;

-- The verse player of an X-Verse-Player key, or NULL.
CREATE FUNCTION makers.player_of(k text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT p.id FROM verse.players p WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND p.key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex')
$$;

CREATE FUNCTION makers.owns(pl text, l bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT pl IS NOT NULL AND EXISTS (SELECT 1 FROM makers.tips t WHERE t.listing = l AND t.buyer = pl AND t.paid IS NOT NULL)
$$;

-- A listing as players see it (never its maker's address).
CREATE FUNCTION makers.listing_json(l makers.listings, pl text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('id', l.id, 'name', l.name, 'by', l.maker_name, 'sats', l.sats, 'look', l.look,
                            'state', l.state, 'owned', makers.owns(pl, l.id))
$$;

-- A maker lists a fairy: body {"name", "by", "look", "sats", "lightning"} from the logged-in key pk.
-- It waits for an admin. {"status": 200, "id", "state": "pending"}, or 400, 429 (five waiting already).
CREATE FUNCTION makers.listing_new(pk text, body text, min_sats bigint, max_sats bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb; n text; byn text; lk text; a text; s numeric; l makers.listings;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' OR pk !~ '^[0-9a-f]{64}$' THEN RETURN '{"status":400,"error":"not JSON"}'; END IF;
  n := makers.name_of(b->>'name');
  IF n IS NULL THEN RETURN '{"status":400,"error":"name your fairy, up to 40 letters"}'; END IF;
  byn := makers.name_of(b->>'by');
  IF byn IS NULL THEN RETURN '{"status":400,"error":"type your name, up to 40 letters"}'; END IF;
  lk := makers.look_of(b->>'look');
  IF lk IS NULL THEN RETURN '{"status":400,"error":"that isn''t a fairy look code"}'; END IF;
  a := makers.address_of(b->>'lightning');
  IF a IS NULL THEN RETURN '{"status":400,"error":"that isn''t a Lightning address (name@wallet.com)"}'; END IF;
  IF jsonb_typeof(b->'sats') IS DISTINCT FROM 'number' OR (b->>'sats') !~ '^[1-9][0-9]{0,8}$' THEN
    RETURN '{"status":400,"error":"sats must be a whole number"}';
  END IF;
  s := (b->>'sats')::numeric;
  IF s < min_sats OR s > max_sats THEN RETURN jsonb_build_object('status', 400, 'error', format('a price is %s to %s sats', min_sats, max_sats)); END IF;
  IF (SELECT count(*) FROM makers.listings WHERE maker = pk AND state = 'pending') >= 5 THEN
    RETURN '{"status":429,"error":"five of your listings are waiting already"}';
  END IF;
  INSERT INTO makers.listings (name, look, sats, maker, maker_name, lightning) VALUES (n, lk, s::bigint, pk, byn, a) RETURNING * INTO l;
  RETURN jsonb_build_object('status', 200, 'id', l.id, 'state', l.state);
END $$;

-- An admin approves or rejects a listing. {"status": 200, "id", "state"} or 400 / 404.
CREATE FUNCTION makers.listing_decide(l bigint, admin text, st text) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  IF st NOT IN ('approved', 'rejected') THEN RETURN '{"status":400,"error":"approve or reject"}'; END IF;
  UPDATE makers.listings SET state = st, decided = now(), decided_by = admin WHERE id = l;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"no such listing"}'; END IF;
  RETURN jsonb_build_object('status', 200, 'id', l, 'state', st);
END $$;

-- UPGRADE's answer for a player: the top three approved fairies (most bought by players, then the
-- newest), what the player wears (null: their own), and for an admin the listings waiting.
CREATE FUNCTION makers.upgrades(pl text, is_admin boolean) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('status', 200,
    'top', coalesce((SELECT jsonb_agg(makers.listing_json(x.l, pl) ORDER BY x.sold DESC, x.created DESC, x.id DESC)
                     FROM (SELECT l, l.created, l.id, (SELECT count(*) FROM makers.tips t WHERE t.listing = l.id AND t.paid IS NOT NULL AND NOT t.team) AS sold
                           FROM makers.listings l WHERE l.kind = 'fairy' AND l.state = 'approved'
                           ORDER BY sold DESC, l.created DESC, l.id DESC LIMIT 3) x), '[]'::jsonb),
    'wearing', (SELECT makers.listing_json(l, pl) FROM makers.wearing w JOIN makers.listings l ON l.id = w.listing WHERE w.player = pl),
    'pending', CASE WHEN is_admin THEN coalesce((SELECT jsonb_agg(makers.listing_json(l, pl) ORDER BY l.created, l.id)
                                                FROM makers.listings l WHERE l.id IN (SELECT id FROM makers.listings WHERE state = 'pending' ORDER BY created, id LIMIT 3)), '[]'::jsonb)
               ELSE '[]'::jsonb END,
    'admin', is_admin)
$$;

-- Wears what was bought; listing 0 goes back to the fairy the player made. {"status": 200, "look"} or 403 / 404.
CREATE FUNCTION makers.wear(pl text, l bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  x makers.listings;
BEGIN
  IF pl IS NULL THEN RETURN '{"status":403,"error":"no player"}'; END IF;
  IF l = 0 THEN DELETE FROM makers.wearing w WHERE w.player = pl; RETURN '{"status":200,"look":""}'; END IF;
  SELECT * INTO x FROM makers.listings WHERE id = l;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"no such listing"}'; END IF;
  IF NOT makers.owns(pl, l) THEN RETURN '{"status":403,"error":"buy it first"}'; END IF;
  INSERT INTO makers.wearing (player, listing) VALUES (pl, l) ON CONFLICT (player) DO UPDATE SET listing = excluded.listing, since = now();
  RETURN jsonb_build_object('status', 200, 'look', x.look, 'id', l);
END $$;

-- What buying a listing asks: {"status": 200, "sats", "memo"} or 403 (no player), 404 (not listed), 409 (owned).
CREATE FUNCTION makers.buy_terms(l bigint, pl text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  x makers.listings;
BEGIN
  IF pl IS NULL THEN RETURN '{"status":403,"error":"no player"}'; END IF;
  SELECT * INTO x FROM makers.listings WHERE id = l AND state = 'approved';
  IF NOT FOUND THEN RETURN '{"status":404,"error":"this fairy isn''t listed"}'; END IF;
  IF makers.owns(pl, l) THEN RETURN '{"status":409,"error":"it is yours already"}'; END IF;
  RETURN jsonb_build_object('status', 200, 'sats', x.sats, 'memo', format('%s, a fairy by %s, on OpenAgents', x.name, x.maker_name));
END $$;

-- Keeps our wallet's invoice for a purchase, a tip for the listing's maker. {"status": 200, "invoice",
-- "payment_hash", "sats", "expires"} or 502 when the wallet's answer doesn't fit.
CREATE FUNCTION makers.buy_issue(l bigint, pl text, inv text, hash text, amount_msat bigint, expires_unix bigint, is_team boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  x makers.listings;
BEGIN
  SELECT * INTO x FROM makers.listings WHERE id = l AND state = 'approved';
  IF NOT FOUND OR pl IS NULL THEN RETURN '{"status":404,"error":"this fairy isn''t listed"}'; END IF;
  IF hash !~ '^[0-9a-f]{64}$' OR inv !~ '^lnbc[0-9a-z]+$' OR amount_msat <> x.sats * 1000 OR expires_unix <= extract(epoch FROM now()) THEN
    RETURN '{"status":502,"error":"our wallet''s invoice does not fit"}';
  END IF;
  INSERT INTO makers.tips (payment_hash, invoice, realm, maker, lightning, sats, team, expires, listing, buyer, paid_with)
  VALUES (hash, inv, 'fairy-forest', x.maker, x.lightning, x.sats, coalesce(is_team, false), to_timestamp(expires_unix), l, pl, 'sats');
  RETURN jsonb_build_object('status', 200, 'invoice', inv, 'payment_hash', hash, 'sats', x.sats, 'expires', expires_unix);
END $$;

-- A paid purchase: its payout is an upgrade's, and the buyer wears it.
CREATE FUNCTION makers.bought(t makers.tips) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  UPDATE makers.payouts SET source = 'upgrade' WHERE tip = t.payment_hash;
  INSERT INTO makers.wearing (player, listing) VALUES (t.buyer, t.listing) ON CONFLICT (player) DO UPDATE SET listing = excluded.listing, since = now();
END $$;

-- Settles a purchase paid in sats from our wallet's record (makers.tip_settle: the replay ledger once,
-- the payout queued, no first-tip bonus), then wears it. As tip_settle answers.
CREATE FUNCTION makers.buy_settle(hash text, state text, amount_msat bigint, preimage text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb; t makers.tips;
BEGIN
  SELECT * INTO t FROM makers.tips WHERE payment_hash = hash AND listing IS NOT NULL;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"no such purchase"}'; END IF;
  r := makers.tip_settle(hash, state, amount_msat, preimage, 0);
  IF (r->>'status')::int = 200 AND r->'again' IS NULL THEN
    SELECT * INTO t FROM makers.tips WHERE payment_hash = hash;
    PERFORM makers.bought(t);
  END IF;
  RETURN r;
END $$;

-- Where a purchase stands: {"status": 200, "state": "open"|"paid"|"expired", "sats", "look", "payout"} or 404.
CREATE FUNCTION makers.buy_state(l bigint, hash text, pl text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((
    SELECT jsonb_build_object('status', 200, 'sats', t.sats, 'invoice', t.invoice, 'id', l, 'look', x.look,
             'state', CASE WHEN t.paid IS NOT NULL THEN 'paid' WHEN t.expires < now() THEN 'expired' ELSE 'open' END,
             'payout', coalesce(p.state, ''))
    FROM makers.tips t JOIN makers.listings x ON x.id = t.listing LEFT JOIN makers.payouts p ON p.tip = t.payment_hash
    WHERE t.payment_hash = hash AND t.listing = l AND t.buyer = pl), '{"status":404,"error":"no such purchase"}'::jsonb)
$$;

-- Buys a listing with the logged-in key's inference credit, in one transaction: the credit taken
-- (sats * 1000 msat), the purchase kept paid, the maker's payout queued, the fairy worn.
-- {"status": 200, "state": "paid", "look", "credit_msat"} or 402 (not enough credit), 403, 404, 409.
CREATE FUNCTION makers.buy_credit(l bigint, pl text, pk text, is_team boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  terms jsonb; x makers.listings; c inference.credit; t makers.tips;
BEGIN
  terms := makers.buy_terms(l, pl);
  IF (terms->>'status')::int <> 200 THEN RETURN terms; END IF;
  IF pk IS NULL OR pk !~ '^[0-9a-f]{64}$' THEN RETURN '{"status":401,"error":"log in to pay with credit"}'; END IF;
  SELECT * INTO x FROM makers.listings WHERE id = l;
  SELECT * INTO c FROM inference.credit WHERE pubkey = pk FOR UPDATE;
  IF NOT FOUND OR c.msat < x.sats * 1000 THEN
    RETURN jsonb_build_object('status', 402, 'error', 'not enough credit', 'credit_msat', coalesce(c.msat, 0));
  END IF;
  UPDATE inference.credit SET msat = msat - x.sats * 1000, updated = now() WHERE pubkey = pk RETURNING * INTO c;
  INSERT INTO makers.tips (payment_hash, realm, maker, lightning, sats, team, expires, paid, listing, buyer, buyer_key, paid_with)
  VALUES ('credit:' || md5(random()::text || clock_timestamp()::text || pk), 'fairy-forest', x.maker, x.lightning, x.sats,
          coalesce(is_team, false), now(), now(), l, pl, pk, 'credit') RETURNING * INTO t;
  PERFORM makers.queue_payout(t, 0);
  PERFORM makers.bought(t);
  RETURN jsonb_build_object('status', 200, 'state', 'paid', 'id', l, 'look', x.look, 'credit_msat', c.msat);
END $$;

-- Listings for an admin, a line each: id|state|sats|sold|name|by|look.
CREATE FUNCTION makers.listing_lines() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(format('%s|%s|%s|%s|%s|%s|%s', l.id, l.state, l.sats,
                                    (SELECT count(*) FROM makers.tips t WHERE t.listing = l.id AND t.paid IS NOT NULL), l.name, l.maker_name, l.look),
                             E'\n' ORDER BY l.id), '')
  FROM makers.listings l
$$;

-- As 0074's, and each player wearing a bought fairy carries its look: what everyone else draws.
CREATE OR REPLACE FUNCTION verse.room_seen(r text, me text, pair text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                            || CASE WHEN l.look IS NOT NULL THEN jsonb_build_object('look', l.look) ELSE '{}'::jsonb END
                            ORDER BY h.since_ms, h.player), '[]')
  FROM verse.room_here h
  LEFT JOIN makers.wearing w ON w.player = h.player
  LEFT JOIN makers.listings l ON l.id = w.listing AND l.state = 'approved'
  WHERE h.room = r AND h.seen_ms >= verse.now_ms() - verse.room_present_ms()
    AND (h.auto IS NULL OR (me IS NOT NULL AND h.player = me) OR (pair IS NOT NULL AND h.auto = pair))
$$;
