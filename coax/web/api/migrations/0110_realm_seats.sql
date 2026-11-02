-- A seat in a running realm, opened by its account (docs/verse/realm-seat.md, #12655).
--
-- POST /api/playtest/seats (seats.c) opens one for a signed caller (NIP-98, its account); the
-- limits are coax/web/api/seat-policy.coax's (#authority): one seat an account (opening one closes
-- the account's open one), at most 8 open on the site, closed after 10 minutes not kept. A seat
-- has its own player key, minted here for the caller's account and handed only to the caller (or,
-- hosted, to the runner that holds its page): the seat's page plays with it, so the realm's routes,
-- character chat and presence meet it as that account, through the same rules a page's key meets
-- (verse.owns, verse.realm_hidden). The key works only while the seat is open: closing a seat (its
-- account's close, a newer seat, the idle limit) changes the player's key hash so it never
-- resolves again. A site admin's seat (admin, decided by seats.c from the signer) sees a private
-- realm as an admin does. A seat's presence is an agent's (0108's flag) whether or not it is signed.
CREATE TABLE verse.seats (
  id bigserial PRIMARY KEY,
  account text NOT NULL,
  signer text NOT NULL,             -- the pubkey that opened it
  slug text NOT NULL,
  player text NOT NULL REFERENCES verse.players (id),
  admin boolean NOT NULL DEFAULT false,
  hosted boolean NOT NULL DEFAULT false,
  opened_ms bigint NOT NULL,
  kept_ms bigint NOT NULL,
  closed_ms bigint,
  closed_why text
);
CREATE INDEX seats_open ON verse.seats (account) WHERE closed_ms IS NULL;
CREATE INDEX seats_player ON verse.seats (player) WHERE closed_ms IS NULL;

-- Closes a seat and kills its key (the hash can never be a SHA-256 hex again).
CREATE FUNCTION verse.seat_shut(s verse.seats, why text) RETURNS void LANGUAGE sql AS $$
  UPDATE verse.seats SET closed_ms = verse.now_ms(), closed_why = why WHERE id = s.id AND closed_ms IS NULL;
  UPDATE verse.players SET key_sha256 = 'closed-seat:' || s.id WHERE id = s.player;
$$;

-- The open seats with when each was last kept, and the time now: seats.c asks seat-policy.coax
-- (sp_live) of each and closes those it says aren't (verse.seat_idle).
CREATE FUNCTION verse.seats_kept() RETURNS TABLE (id bigint, kept_ms bigint, now_ms bigint) LANGUAGE sql STABLE AS $$
  SELECT id, kept_ms, verse.now_ms() FROM verse.seats WHERE closed_ms IS NULL ORDER BY id $$;
CREATE FUNCTION verse.seat_idle(id_ bigint) RETURNS void LANGUAGE sql AS $$
  SELECT verse.seat_shut(s, 'idle') FROM verse.seats s WHERE s.id = id_ AND s.closed_ms IS NULL $$;

-- The facts seats.c hands seat-policy.coax before opening one: seats open now on the site but the
-- account's own, and whether the realm is there for the caller (a made realm it may see).
CREATE FUNCTION verse.seat_facts(acct text, k text, slug_ text, site_admin boolean) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'others', (SELECT count(*) FROM verse.seats WHERE closed_ms IS NULL AND account <> acct),
    'seen', EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = slug_) AND NOT verse.realm_hidden(slug_, k, site_admin))
$$;

-- Opens a seat (seat-policy.coax admitted it): closes the account's open ones, mints the seat's
-- player for the account. -> {id, slug, player: {id, key}, until}
CREATE FUNCTION verse.seat_open(acct text, signer_ text, slug_ text, admin_ boolean, hosted_ boolean, until_ms bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  s verse.seats;
  p jsonb;
  id_ bigint;
BEGIN
  FOR s IN SELECT * FROM verse.seats WHERE closed_ms IS NULL AND account = acct LOOP
    PERFORM verse.seat_shut(s, 'replaced');
  END LOOP;
  p := verse.mint(acct, 'realm-seat');
  INSERT INTO verse.seats (account, signer, slug, player, admin, hosted, opened_ms, kept_ms)
  VALUES (acct, signer_, slug_, p->>'id', admin_, hosted_, verse.now_ms(), verse.now_ms()) RETURNING id INTO id_;
  RETURN jsonb_build_object('id', id_, 'slug', slug_, 'hosted', hosted_, 'player', jsonb_build_object('id', p->>'id', 'key', p->>'key'), 'until', until_ms);
END $$;

-- The account's open seat id_, or none.
CREATE FUNCTION verse.seat_mine(id_ bigint, acct text) RETURNS verse.seats LANGUAGE sql STABLE AS $$
  SELECT * FROM verse.seats WHERE id = id_ AND account = acct AND closed_ms IS NULL $$;

-- Kept now (an action): 200, or 404 when it isn't the account's open seat.
CREATE FUNCTION verse.seat_keep(id_ bigint, acct text) RETURNS int LANGUAGE sql AS $$
  WITH k AS (UPDATE verse.seats SET kept_ms = verse.now_ms() WHERE id = id_ AND account = acct AND closed_ms IS NULL RETURNING 1)
  SELECT CASE WHEN EXISTS (SELECT 1 FROM k) THEN 200 ELSE 404 END $$;

-- Closed by its account: 200, or 404.
CREATE FUNCTION verse.seat_close(id_ bigint, acct text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE s verse.seats := verse.seat_mine(id_, acct);
BEGIN
  IF s.id IS NULL THEN RETURN 404; END IF;
  PERFORM verse.seat_shut(s, 'closed');
  RETURN 200;
END $$;

-- The account's open seats, newest first.
CREATE FUNCTION verse.seats_mine(acct text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'slug', slug, 'hosted', hosted, 'opened_ms', opened_ms, 'kept_ms', kept_ms) ORDER BY id DESC), '[]')
  FROM verse.seats WHERE account = acct AND closed_ms IS NULL $$;

-- Whether a player is an open seat's.
CREATE FUNCTION verse.seat_player(pid text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT pid IS NOT NULL AND EXISTS (SELECT 1 FROM verse.seats WHERE player = pid AND closed_ms IS NULL) $$;

-- As 0099's, and a site admin's open seat sees a private realm as the admin does.
CREATE OR REPLACE FUNCTION verse.realm_hidden(s text, k text, site_admin boolean) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.made_realms m WHERE m.slug = s AND m.visibility = 'private')
     AND NOT coalesce(site_admin, false) AND NOT verse.builder_owns(k, s)
     AND NOT EXISTS (SELECT 1 FROM verse.seats st JOIN verse.players p ON p.id = st.player
                     WHERE st.admin AND st.closed_ms IS NULL AND k ~ '^[A-Za-z0-9_-]{20,128}$'
                       AND p.key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex')) $$;

-- As 0108's, and a seat's page posting with its seat's key is an agent's presence too.
CREATE OR REPLACE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text,
                                      seat boolean, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  me text;
  agent_ boolean;
BEGIN
  IF verse.room_hidden(r, k, site_admin) THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  IF NOT coalesce(seat, false) THEN
    out := verse.room(method, kind, r, body, k, after, mark, ua);
    IF kind = 'presence' AND method = 'POST' AND (out->>'status')::int = 200 THEN
      me := out->'body'->'you'->>'id';
      agent_ := verse.seat_player(me);
      UPDATE verse.room_here SET agent = agent_ WHERE room = r AND player = me AND agent IS DISTINCT FROM agent_;
      IF agent_ THEN out := jsonb_set(out, '{body,you,agent}', 'true'::jsonb); END IF;
    END IF;
    RETURN out;
  END IF;
  IF kind <> 'presence' OR method <> 'POST' THEN RETURN verse.r(405, '{"error":"a seat posts its presence"}'); END IF;
  IF jsonb_typeof(body) = 'object' AND body->'gone' = 'true'::jsonb THEN
    SELECT id INTO me FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
    IF me IS NULL THEN RETURN verse.r(403, '{"error":"no player"}'); END IF;
    DELETE FROM verse.room_here WHERE room = r AND player = me;
    RETURN verse.r(200, '{"gone": true}');
  END IF;
  out := verse.room(method, kind, r, body, k, after, 'seat', ua);
  IF (out->>'status')::int <> 200 THEN RETURN out; END IF;
  UPDATE verse.room_here SET agent = true,
    seat_page = CASE WHEN jsonb_typeof(body) = 'object' AND body->>'page' ~ '^p-[a-z0-9]{1,40}$' THEN body->>'page' END
  WHERE room = r AND player = out->'body'->'you'->>'id';
  RETURN jsonb_set(out, '{body,you,agent}', 'true'::jsonb);
END $$;
