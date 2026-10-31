-- A realm's maker (ev-claim-realm, docs/verse/makers.md): the person a realm's tips go to.
-- An admin makes a one-time claim code for a realm (makers.c, POST /api/admin/makers/<realm>/code);
-- the owner sends its link (/realms/<realm>/claim#code=...); the maker opens it on their phone,
-- logs in with their Nostr key (/login's words account, or the phone app), and sets a display
-- name and a Lightning address. The realm page then says "Made by <name>".
--
-- Kept about the person: their pubkey, the name and the Lightning address they typed. Nothing
-- else. Only SHA-256 of a code is stored, and a code works once. Additive: nothing here
-- changes another table.

CREATE SCHEMA IF NOT EXISTS makers;

CREATE TABLE makers.codes (
  code_sha256 text PRIMARY KEY,
  realm text NOT NULL,
  made_by text NOT NULL,                       -- the admin's pubkey
  made timestamptz NOT NULL DEFAULT now(),
  used timestamptz,
  used_by text                                 -- the pubkey that claimed with it
);
CREATE INDEX makers_codes_realm ON makers.codes (realm, made);

CREATE TABLE makers.realms (
  realm text PRIMARY KEY,
  pubkey text NOT NULL,
  name text NOT NULL,
  lightning text NOT NULL,
  claimed timestamptz NOT NULL DEFAULT now(),
  updated timestamptz NOT NULL DEFAULT now()
);

-- A realm's slug: what /realms/<slug> takes.
CREATE FUNCTION makers.slug_ok(r text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT r ~ '^[a-z0-9][a-z0-9-]{0,63}$'
$$;

-- A display name: 1 to 40 characters once trimmed, no control characters, no '|' (lines answers).
CREATE FUNCTION makers.name_of(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN btrim(t) <> '' AND char_length(btrim(t)) <= 40 AND btrim(t) !~ '[[:cntrl:]|<>]' THEN btrim(t) END
$$;

-- A Lightning address (LUD-16): user@domain, lowercased, at most 120 characters.
CREATE FUNCTION makers.address_of(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN lower(btrim(t)) ~ '^[a-z0-9._+-]{1,64}@([a-z0-9-]{1,63}\.)+[a-z]{2,63}$' AND char_length(btrim(t)) <= 120
         THEN lower(btrim(t)) END
$$;

-- An admin's new code for a realm: the code is the caller's (32 hex digits from /dev/urandom);
-- only its hash is kept. {"status": 200, "realm"} or 400.
CREATE FUNCTION makers.make_code(r text, code text, admin text) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  IF NOT makers.slug_ok(r) THEN RETURN '{"status":400,"error":"not a realm"}'; END IF;
  IF code !~ '^[0-9a-f]{32}$' THEN RETURN '{"status":400,"error":"bad code"}'; END IF;
  INSERT INTO makers.codes (code_sha256, realm, made_by) VALUES (encode(sha256(convert_to(code, 'UTF8')), 'hex'), r, admin);
  RETURN jsonb_build_object('status', 200, 'realm', r);
END $$;

-- A claim: body {"code", "name", "lightning"} from the logged-in key pk. The code must be this
-- realm's and unused; it is spent in the same transaction that names the maker. A later code for
-- the same realm hands it to whoever claims with it. {"status": 200, "realm", "name", "lightning"},
-- or 400 (a bad name or address), 404 (no such code for this realm), 409 (the code was used).
CREATE FUNCTION makers.claim(r text, pk text, body text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb; c makers.codes; n text; a text;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' OR jsonb_typeof(b->'code') IS DISTINCT FROM 'string' THEN
    RETURN '{"status":400,"error":"this link has no code"}';
  END IF;
  IF NOT makers.slug_ok(r) OR pk !~ '^[0-9a-f]{64}$' THEN RETURN '{"status":400,"error":"not a realm"}'; END IF;
  n := makers.name_of(b->>'name');
  IF n IS NULL THEN RETURN '{"status":400,"error":"type your name, up to 40 letters"}'; END IF;
  a := makers.address_of(b->>'lightning');
  IF a IS NULL THEN RETURN '{"status":400,"error":"that isn''t a Lightning address (name@wallet.com)"}'; END IF;
  SELECT * INTO c FROM makers.codes WHERE code_sha256 = encode(sha256(convert_to(b->>'code', 'UTF8')), 'hex') FOR UPDATE;
  IF NOT FOUND OR c.realm <> r THEN RETURN '{"status":404,"error":"this claim link isn''t for this realm"}'; END IF;
  IF c.used IS NOT NULL THEN RETURN '{"status":409,"error":"this claim link was used already"}'; END IF;
  UPDATE makers.codes SET used = now(), used_by = pk WHERE code_sha256 = c.code_sha256;
  INSERT INTO makers.realms (realm, pubkey, name, lightning) VALUES (r, pk, n, a)
  ON CONFLICT (realm) DO UPDATE SET pubkey = excluded.pubkey, name = excluded.name, lightning = excluded.lightning,
    claimed = now(), updated = now();
  RETURN jsonb_build_object('status', 200, 'realm', r, 'name', n, 'lightning', a);
END $$;

-- The maker changes their name or address (no code: the key that claimed the realm).
-- {"status": 200, ...} or 400, 403 (not this realm's maker), 404 (unclaimed).
CREATE FUNCTION makers.edit(r text, pk text, body text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  b jsonb; m makers.realms; n text; a text;
BEGIN
  BEGIN b := body::jsonb; EXCEPTION WHEN others THEN RETURN '{"status":400,"error":"not JSON"}'; END;
  SELECT * INTO m FROM makers.realms WHERE realm = r FOR UPDATE;
  IF NOT FOUND THEN RETURN '{"status":404,"error":"nobody has claimed this realm"}'; END IF;
  IF m.pubkey <> pk THEN RETURN '{"status":403,"error":"only its maker can change this"}'; END IF;
  IF jsonb_typeof(b) IS DISTINCT FROM 'object' THEN RETURN '{"status":400,"error":"not JSON"}'; END IF;
  IF b ? 'name' AND makers.name_of(b->>'name') IS NULL THEN RETURN '{"status":400,"error":"type your name, up to 40 letters"}'; END IF;
  IF b ? 'lightning' AND makers.address_of(b->>'lightning') IS NULL THEN
    RETURN '{"status":400,"error":"that isn''t a Lightning address (name@wallet.com)"}';
  END IF;
  n := coalesce(makers.name_of(b->>'name'), m.name);
  a := coalesce(makers.address_of(b->>'lightning'), m.lightning);
  UPDATE makers.realms SET name = n, lightning = a, updated = now() WHERE realm = r;
  RETURN jsonb_build_object('status', 200, 'realm', r, 'name', n, 'lightning', a);
END $$;

-- Who made a realm, for anyone: {"status": 200, "realm", "name", "pubkey"} (never the address), or 404.
-- The maker themselves (pk) also gets their address back, to check it.
CREATE FUNCTION makers.of(r text, pk text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT jsonb_build_object('status', 200, 'realm', m.realm, 'name', m.name, 'pubkey', m.pubkey)
                          || CASE WHEN pk = m.pubkey THEN jsonb_build_object('lightning', m.lightning, 'mine', true) ELSE '{}'::jsonb END
                   FROM makers.realms m WHERE m.realm = r),
                  '{"status":404,"error":"nobody has claimed this realm"}'::jsonb)
$$;
