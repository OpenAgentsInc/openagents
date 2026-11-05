-- Durable parties (#12852; epic #12833): a group of players that outlives a room, a session and a
-- restart, its members' health seen across rooms, and placement that keeps it together.
--
-- A party is a row of verse.parties and its members' rows of verse.party_members (one party a
-- player). Every change to who is in it (a join, a leave, a hide) raises its version, so a page
-- holding an older roster knows it is stale.
--   POST /api/playtest/party/invite         -> {party, code, until}: a code that lets one player join
--                                              your party for PARTY_INVITE_MS (a party of one is made)
--   POST /api/playtest/party/join  {code}   -> {party, version}; 404 no such code or over, 403 blocked
--                                              (either way), 409 the party is full (PARTY_MOST)
--   POST /api/playtest/party/leave          -> {party: null}; the last member out ends the party
--   POST /api/playtest/party/hide  {on}     -> your health is left out of the others' rosters
--   GET  /api/playtest/party                -> the roster: {party, version, until, members: [{id, name,
--                                              hp, hp_max, you}]}, health only
-- The roster is a subscription that expires: the page asks again before UNTIL (PARTY_SEEN_MS), and
-- a version it has already shown changes nothing. It carries health only: no place, no room, no
-- look, nothing of the room's simulation. HP is the room's own last report (verse.room_vitals, its
-- newest owner's: 0127), null when that is older than PARTY_HP_MS (the room stopped reporting).
-- A member you block, or who blocks you (verse.player_blocks), and a hidden member are not in your
-- roster. Placement (verse.party_room, rooms.c): a page asking for a realm without naming a room
-- goes to the room its party sits in while it has a seat; when every such room is full, the ticket
-- says so ("party_full") and the page is placed elsewhere.
-- Additive, and safe to run twice.

CREATE OR REPLACE FUNCTION verse.party_most() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 8 $$;
CREATE OR REPLACE FUNCTION verse.party_invite_ms() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 600000::bigint $$;
CREATE OR REPLACE FUNCTION verse.party_seen_ms() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 60000::bigint $$;
CREATE OR REPLACE FUNCTION verse.party_hp_ms() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 600000::bigint $$;

CREATE TABLE IF NOT EXISTS verse.parties (
  id bigserial PRIMARY KEY,
  version int NOT NULL DEFAULT 1,
  made_ms bigint NOT NULL
);
CREATE TABLE IF NOT EXISTS verse.party_members (
  pid text PRIMARY KEY REFERENCES verse.players (id),
  party bigint NOT NULL REFERENCES verse.parties (id) ON DELETE CASCADE,
  hidden boolean NOT NULL DEFAULT false,
  joined_ms bigint NOT NULL
);
CREATE INDEX IF NOT EXISTS party_members_party ON verse.party_members (party);
CREATE TABLE IF NOT EXISTS verse.party_invites (
  code text PRIMARY KEY,
  party bigint NOT NULL REFERENCES verse.parties (id) ON DELETE CASCADE,
  by_pid text NOT NULL,
  until_ms bigint NOT NULL
);

-- Whether a and b block each other, either way.
CREATE OR REPLACE FUNCTION verse.party_blocked(a text, b text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.player_blocks x WHERE (x.player = a AND x.blocked = b) OR (x.player = b AND x.blocked = a)) $$;

-- Player me leaves their party, if any; the last one out ends it.
CREATE OR REPLACE FUNCTION verse.party_leave(me text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE p bigint;
BEGIN
  DELETE FROM verse.party_members m WHERE m.pid = me RETURNING m.party INTO p;
  IF p IS NULL THEN RETURN; END IF;
  IF NOT EXISTS (SELECT 1 FROM verse.party_members m WHERE m.party = p) THEN DELETE FROM verse.parties x WHERE x.id = p;
  ELSE UPDATE verse.parties x SET version = x.version + 1 WHERE x.id = p; END IF;
END $$;

-- A code for one join of me's party (a party of one made when me has none), and its end.
CREATE OR REPLACE FUNCTION verse.party_invite(me text, OUT party bigint, OUT code text, OUT until_ms bigint)
LANGUAGE plpgsql AS $$
DECLARE now bigint := (extract(epoch FROM clock_timestamp()) * 1000)::bigint;
BEGIN
  SELECT m.party INTO party FROM verse.party_members m WHERE m.pid = me;
  IF party IS NULL THEN
    INSERT INTO verse.parties (made_ms) VALUES (now) RETURNING id INTO party;
    INSERT INTO verse.party_members (pid, party, joined_ms) VALUES (me, party, now);
  END IF;
  DELETE FROM verse.party_invites i WHERE i.until_ms <= now;
  code := replace(gen_random_uuid()::text, '-', '');
  until_ms := now + verse.party_invite_ms();
  INSERT INTO verse.party_invites (code, party, by_pid, until_ms) VALUES (code, party, me, until_ms);
END $$;

-- Player me joins by an invite's code: its party and version, or why not: 'gone' (no such code,
-- or over), 'blocked', 'full'. A code is used once. me leaves the party it was in.
CREATE OR REPLACE FUNCTION verse.party_join(me text, given text, OUT party bigint, OUT version int, OUT why text)
LANGUAGE plpgsql AS $$
DECLARE
  now bigint := (extract(epoch FROM clock_timestamp()) * 1000)::bigint;
  inv verse.party_invites;
BEGIN
  SELECT * INTO inv FROM verse.party_invites i WHERE i.code = given AND i.until_ms > now FOR UPDATE;
  IF NOT FOUND THEN why := 'gone'; RETURN; END IF;
  PERFORM 1 FROM verse.parties x WHERE x.id = inv.party FOR UPDATE;
  IF NOT FOUND THEN why := 'gone'; RETURN; END IF;
  IF EXISTS (SELECT 1 FROM verse.party_members m WHERE m.pid = me AND m.party = inv.party) THEN
    SELECT x.version INTO version FROM verse.parties x WHERE x.id = inv.party; party := inv.party; RETURN;
  END IF;
  IF EXISTS (SELECT 1 FROM verse.party_members m WHERE m.party = inv.party AND verse.party_blocked(m.pid, me)) THEN why := 'blocked'; RETURN; END IF;
  IF (SELECT count(*) FROM verse.party_members m WHERE m.party = inv.party) >= verse.party_most() THEN why := 'full'; RETURN; END IF;
  PERFORM verse.party_leave(me);
  DELETE FROM verse.party_invites i WHERE i.code = given;
  INSERT INTO verse.party_members (pid, party, joined_ms) VALUES (me, inv.party, now);
  UPDATE verse.parties x SET version = x.version + 1 WHERE x.id = inv.party RETURNING x.version INTO version;
  party := inv.party;
END $$;

-- me's health hidden from the others' rosters (on) or shown again. False: me is in no party.
CREATE OR REPLACE FUNCTION verse.party_hide(me text, on_ boolean) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE p bigint;
BEGIN
  UPDATE verse.party_members m SET hidden = on_ WHERE m.pid = me AND m.hidden <> on_ RETURNING m.party INTO p;
  IF p IS NOT NULL THEN UPDATE verse.parties x SET version = x.version + 1 WHERE x.id = p; END IF;
  RETURN EXISTS (SELECT 1 FROM verse.party_members m WHERE m.pid = me);
END $$;

-- me's roster, health only (above). {"party":null} when me is in no party.
CREATE OR REPLACE FUNCTION verse.party_view(me text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint AS t),
  mine AS (SELECT m.party, x.version FROM verse.party_members m JOIN verse.parties x ON x.id = m.party WHERE m.pid = me),
  rows AS (
    SELECT m.pid, m.joined_ms, m.pid = me AS you,
           CASE WHEN rv.hp_at > clk.t - verse.party_hp_ms() THEN rv.hp END AS hp,
           CASE WHEN rv.hp_at > clk.t - verse.party_hp_ms() THEN rv.hp_max END AS hp_max
    FROM mine JOIN verse.party_members m ON m.party = mine.party
    JOIN verse.players p ON p.id = m.pid
    LEFT JOIN verse.room_vitals rv ON rv.who = coalesce(p.account, p.id), clk
    WHERE m.pid = me OR (NOT m.hidden AND NOT verse.party_blocked(m.pid, me)))
  SELECT CASE WHEN NOT EXISTS (SELECT 1 FROM mine) THEN '{"party":null}' ELSE
    format('{"party":%s,"version":%s,"until":%s,"members":[%s]}', (SELECT party FROM mine), (SELECT version FROM mine),
           (SELECT t FROM clk) + verse.party_seen_ms(),
           coalesce((SELECT string_agg(format('{"id":%s,"name":%s,"hp":%s,"hp_max":%s,"you":%s}', to_json(pid),
                                              to_json(verse.player_name(pid)), coalesce(hp::text, 'null'),
                                              coalesce(hp_max::text, 'null'), CASE WHEN you THEN 'true' ELSE 'false' END), ',' ORDER BY joined_ms, pid) FROM rows), ''))
  END $$;

-- Where me's party sits in REALM: the live room with the fewest taken seats among those another
-- member is in, and whether every such room is full (is_full) (then CODE names the first of them).
CREATE OR REPLACE FUNCTION verse.party_room(realm text, me text, seats int, OUT code text, OUT is_full boolean)
LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT verse.room_now() AS t),
  mates AS (SELECT m.pid FROM verse.party_members m
            WHERE m.pid <> me AND m.party = (SELECT party FROM verse.party_members WHERE pid = me)
              AND NOT verse.party_blocked(m.pid, me)),
  rooms AS (SELECT i.code, verse.room_taken(i, clk.t) AS taken FROM verse.room_instances i, clk
            WHERE i.realm = party_room.realm AND verse.room_live(i, clk.t) AND i.pids && ARRAY(SELECT pid FROM mates)),
  open AS (SELECT r.code FROM rooms r WHERE r.taken < seats ORDER BY r.taken, r.code LIMIT 1)
  SELECT coalesce((SELECT code FROM open), (SELECT r.code FROM rooms r ORDER BY r.code LIMIT 1)),
         NOT EXISTS (SELECT 1 FROM open) AND EXISTS (SELECT 1 FROM rooms) $$;
