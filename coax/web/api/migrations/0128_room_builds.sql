-- Room builds in the directory (#12839): each shard's build, so a page is placed on a shard that
-- runs its build, and a saved character names the build that saved it.
--
-- A room names its build in the stats it posts every 10 s (coax/web/room/room-stats.h):
--   build engine=E rules=R content=C wire=LO-HI schema=S
-- verse.room_stats_add keeps that on room_shards (build "E.R.C", wire, schema, build_at). A build
-- older than 60 s is unknown: the shard scaled to zero, or its stats stopped.
--
-- verse.room_pick with BUILD ("E.R.C", the page's; '' unknown) places a page with a known build on
-- an instance whose shard runs that build, or whose build is unknown. An instance on a shard that
-- runs another build is passed over: an asked one, a reconnect's, the lowest public one. A new
-- instance goes on the lowest shard under its cap that runs the build or whose build is unknown.
-- When no shard runs the build and none is unknown, the page is placed as before, and the room
-- refuses its hello ("r 0 version", room-everglade.coax eg_admit); the page reloads into the
-- room's build. A page that names no build is placed as before.
--
-- room_vitals.build: the build of the shard whose line was last applied ("E.R.C/S", S the saved-data
-- schema), so a recovery after a rollback can tell which build wrote what it reads.
-- Additive, and safe to run twice.
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS build text;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS wire text;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS schema int;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS build_at bigint;
ALTER TABLE verse.room_vitals ADD COLUMN IF NOT EXISTS build text;

CREATE OR REPLACE FUNCTION verse.room_stats_add(b text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  t bigint := nullif(substring(b FROM '^t ([0-9]{1,15})\n'), '')::bigint;
  s int := nullif(substring(b FROM '\nshard=([0-9]{1,2})\n'), '')::int;
  m text[] := regexp_match(b, '\nbuild engine=([0-9]{1,9}) rules=([0-9]{1,9}) content=([0-9]{1,9}) wire=([0-9]{1,6})-([0-9]{1,6}) schema=([0-9]{1,6})\n');
BEGIN
  IF t IS NULL OR s IS NULL OR s < 1 OR s > 99 OR abs(t - (extract(epoch FROM now()) * 1000)::bigint) > 120000 THEN RETURN NULL; END IF;
  INSERT INTO verse.room_stats (shard, body) VALUES (s, b);
  DELETE FROM verse.room_stats WHERE at < now() - interval '1 day';
  IF m IS NOT NULL THEN
    INSERT INTO verse.room_shards (shard, rooms_cap, build, wire, schema, build_at)
    VALUES (s, 0, m[1] || '.' || m[2] || '.' || m[3], m[4] || '-' || m[5], m[6]::int, t)
    ON CONFLICT (shard) DO UPDATE SET build = EXCLUDED.build, wire = EXCLUDED.wire, schema = EXCLUDED.schema, build_at = EXCLUDED.build_at
    WHERE verse.room_shards.build_at IS NULL OR verse.room_shards.build_at <= EXCLUDED.build_at;
  END IF;
  RETURN s;
END $$;

-- Whether shard k may take a page of build bld: no build named, or the shard's build is unknown
-- (none reported in the last 60 s), or it is that build.
CREATE OR REPLACE FUNCTION verse.room_fits(k int, bld text, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce(bld, '') = '' OR NOT EXISTS (
    SELECT 1 FROM verse.room_shards s WHERE s.shard = k AND s.build IS NOT NULL AND s.build_at > now - 60000 AND s.build <> bld) $$;

-- Whether any shard may take a page of build bld (else it is placed as if it named none).
CREATE OR REPLACE FUNCTION verse.room_any_fits(bld text, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.room_shards s WHERE verse.room_fits(s.shard, bld, now)) $$;

-- room_place for build bld: the lowest shard under its cap that fits it.
CREATE OR REPLACE FUNCTION verse.room_place(now bigint, bld text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT s.shard FROM verse.room_shards s
  WHERE s.rooms_cap > (SELECT count(*) FROM verse.room_instances i WHERE i.shard = s.shard)
    AND (s.free IS NULL OR coalesce(s.at, 0) <= now - 15000
         OR s.free > (SELECT count(*) FROM verse.room_instances i WHERE i.shard = s.shard AND i.at IS NULL))
    AND verse.room_fits(s.shard, bld, now)
  ORDER BY s.shard LIMIT 1 $$;

-- Where a player sits, as the five-argument room_pick (0125), on a shard that fits BUILD.
CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats int, via text, build text,
                                           OUT code text, OUT shard int, OUT was_full boolean)
LANGUAGE plpgsql AS $$
DECLARE
  now bigint := verse.room_now();
  i verse.room_instances;
  k int;
  bld text := build;
BEGIN
  was_full := false;
  PERFORM pg_advisory_xact_lock(7731028);
  PERFORM verse.room_sweep(now);
  IF NOT verse.room_any_fits(bld, now) THEN bld := ''; END IF;
  IF asked <> realm THEN
    SELECT * INTO i FROM verse.room_instances x WHERE x.code = asked;
    IF FOUND AND NOT verse.room_fits(i.shard, bld, now) THEN
      -- the asked instance runs another build: placed as a pick of the realm
      NULL;
    ELSIF FOUND AND (pid = ANY (i.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = room_pick.pid)) THEN
      code := i.code; shard := i.shard;
    ELSIF FOUND AND coalesce(via, '') <> '' AND via <> pid
          AND EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = 'i-' || via AND h.until_ms > now) THEN
      DELETE FROM verse.room_holds h WHERE h.code = i.code AND h.pid = 'i-' || via;
      code := i.code; shard := i.shard;
    ELSIF FOUND AND verse.room_taken(i, now) < seats THEN
      code := i.code; shard := i.shard;
    ELSIF FOUND THEN
      was_full := true;
    ELSE
      k := verse.room_place(now, bld);
      IF k IS NULL THEN RETURN; END IF;
      INSERT INTO verse.room_instances (code, realm, n, shard, made_at)
      VALUES (asked, realm, CASE WHEN asked ~ ('^' || realm || '-[1-9][0-9]{0,2}$') THEN substr(asked, length(realm) + 2)::int END, k, now);
      code := asked; shard := k;
    END IF;
  END IF;
  IF code IS NULL THEN
    -- a reconnect keeps its instance, when it fits
    SELECT x.code, x.shard INTO code, shard FROM verse.room_instances x
    WHERE x.realm = room_pick.realm AND x.n IS NOT NULL AND verse.room_fits(x.shard, bld, now)
      AND (pid = ANY (x.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = x.code AND h.pid = room_pick.pid AND h.until_ms > now))
    ORDER BY x.n LIMIT 1;
  END IF;
  IF code IS NULL THEN
    SELECT x.code, x.shard INTO code, shard FROM verse.room_instances x
    WHERE x.realm = room_pick.realm AND x.n IS NOT NULL AND verse.room_taken(x, now) < seats AND verse.room_fits(x.shard, bld, now)
    ORDER BY x.n LIMIT 1;
  END IF;
  IF code IS NULL THEN
    k := verse.room_place(now, bld);
    IF k IS NULL THEN RETURN; END IF;
    SELECT min(g) INTO code FROM generate_series(1, 999) g
    WHERE NOT EXISTS (SELECT 1 FROM verse.room_instances x WHERE x.realm = room_pick.realm AND x.n = g)
      AND NOT EXISTS (SELECT 1 FROM verse.room_instances x WHERE x.code = room_pick.realm || '-' || g);
    IF code IS NULL THEN RETURN; END IF;
    INSERT INTO verse.room_instances (code, realm, n, shard, made_at) VALUES (realm || '-' || code, realm, code::int, k, now);
    code := realm || '-' || code; shard := k;
  END IF;
  INSERT INTO verse.room_holds (code, pid, until_ms) VALUES (code, pid, now + 20000)
  ON CONFLICT ON CONSTRAINT room_holds_pkey DO UPDATE SET until_ms = EXCLUDED.until_ms;
END $$;

-- room_vital_apply with the owner fence (0127), and the build of the shard that wrote an applied line.
CREATE OR REPLACE FUNCTION verse.room_vital_apply(p_shard int, p_gen bigint, p_pid text, p_hp int, p_max int, p_xp int,
                                                  p_wanted int, p_rev bigint, p_at bigint, p_owner bigint) RETURNS text
LANGUAGE plpgsql AS $$
DECLARE
  newest bigint;
  res text;
BEGIN
  IF p_owner IS NOT NULL THEN
    SELECT o.gen INTO newest FROM verse.room_owners o JOIN verse.players p ON o.who = coalesce(p.account, p.id) WHERE p.id = p_pid;
    IF newest IS NOT NULL AND p_owner < newest THEN RETURN 'f'; END IF;
  END IF;
  res := verse.room_vital_apply(p_shard, p_gen, p_pid, p_hp, p_max, p_xp, p_wanted, p_rev, p_at);
  IF res = 'a' THEN
    UPDATE verse.room_vitals rv SET build = s.build || '/' || s.schema
    FROM verse.room_shards s, verse.players p
    WHERE s.shard = p_shard AND s.build IS NOT NULL AND p.id = p_pid AND rv.who = coalesce(p.account, p.id);
  END IF;
  RETURN res;
END $$;
