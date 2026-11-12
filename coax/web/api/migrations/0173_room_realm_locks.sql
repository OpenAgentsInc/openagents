-- The room directory's lock, by realm (#13300, AAA P5.6). Every placement, report and invite took one
-- global advisory lock (7731028: 0115, 0125, 0128, 0133), so a slow one held every realm's joins.
-- Now:
--   a pick or an invite locks its realm (7731028, hash of the realm), and the asked instance's own
--     name when that belongs to another;
--   a shard's report locks the shard (7731030, shard), then each realm it can change;
--   whoever changes how many rooms a shard holds (a pick that makes an instance, a report) also takes
--     the placement lock (7731029, 0), last.
-- Every caller takes its realm locks in key order (verse.room_lock) and the placement lock last, so
-- no two wait on each other. What each lock guards is what the global one did: a room's seats are
-- counted and held under its realm's lock, a shard's room count under the placement lock.
-- A pick sweeps its own realm only (verse.room_sweep_realm).
--
-- And the World Builder's anti-flood bucket (worldbuilder-policy.coax wb_take) moves from each
-- process's memory to a row here, so it is one bucket an address for every worker and instance.

CREATE OR REPLACE FUNCTION verse.room_lock_key(code text) RETURNS int LANGUAGE sql IMMUTABLE AS $$
  SELECT hashtext(coalesce(substring(code FROM '^(.*)-[1-9][0-9]{0,2}$'), code)) $$;

CREATE OR REPLACE FUNCTION verse.room_lock(keys int[]) RETURNS void LANGUAGE plpgsql AS $$
DECLARE k int;
BEGIN
  FOR k IN SELECT DISTINCT u FROM unnest(keys) u WHERE u IS NOT NULL ORDER BY 1 LOOP
    PERFORM pg_advisory_xact_lock(7731028, k);
  END LOOP;
END $$;

CREATE OR REPLACE FUNCTION verse.room_sweep_realm(realm_ text, now bigint) RETURNS void LANGUAGE sql AS $$
  DELETE FROM verse.room_holds h WHERE h.until_ms <= now
    AND h.code IN (SELECT x.code FROM verse.room_instances x WHERE x.realm = realm_);
  DELETE FROM verse.room_instances i WHERE i.realm = realm_ AND NOT verse.room_live(i, now);
$$;

CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats integer, via text, build text, OUT code text, OUT shard integer, OUT was_full boolean) RETURNS record
    LANGUAGE plpgsql
    AS $_$
DECLARE
  now bigint := verse.room_now();
  i verse.room_instances;
  k int;
  bld text := build;
BEGIN
  was_full := false;
  -- this realm's lock, and the asked instance's when its name belongs to another (in key order)
  PERFORM verse.room_lock(ARRAY[hashtext(realm), verse.room_lock_key(asked)]);
  PERFORM verse.room_sweep_realm(realm, now);
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
      PERFORM pg_advisory_xact_lock(7731029, 0);   -- a new instance: the shards' counts, one placement at a time
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
    PERFORM pg_advisory_xact_lock(7731029, 0);
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
END $_$;

-- The older signatures place as the newest does.
CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats integer, via text, OUT code text, OUT shard integer, OUT was_full boolean) RETURNS record
    LANGUAGE sql AS $$ SELECT * FROM verse.room_pick(realm, asked, pid, seats, via, '') $$;
CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats integer, OUT code text, OUT shard integer, OUT was_full boolean) RETURNS record
    LANGUAGE sql AS $$ SELECT * FROM verse.room_pick(realm, asked, pid, seats, '', '') $$;

CREATE OR REPLACE FUNCTION verse.room_report(body text) RETURNS integer
    LANGUAGE plpgsql
    AS $_$
DECLARE
  now bigint := verse.room_now();
  lines text[] := string_to_array(rtrim(body, E'\n'), E'\n');
  f text[]; at bigint; sh int; fr int; named text[] := '{}'; ln text; m int := 0;
  g bigint; d boolean := false; first int := 3; newest bigint;
BEGIN
  IF array_length(lines, 1) IS NULL OR array_length(lines, 1) < 2 THEN RETURN -1; END IF;
  IF lines[1] !~ '^t [0-9]{1,15}$' OR lines[2] !~ '^s [1-9][0-9]? [0-9]{1,3}$' THEN RETURN -1; END IF;
  at := split_part(lines[1], ' ', 2)::bigint;
  IF at < now - 120000 OR at > now + 120000 THEN RETURN -1; END IF;
  sh := split_part(lines[2], ' ', 2)::int; fr := split_part(lines[2], ' ', 3)::int;
  IF array_length(lines, 1) >= 3 AND lines[3] ~ '^g ' THEN
    IF lines[3] !~ '^g [0-9]{1,15}( d)?$' THEN RETURN -1; END IF;
    g := split_part(lines[3], ' ', 2)::bigint; d := lines[3] ~ ' d$'; first := 4;
  END IF;
  -- this shard's reports one at a time; then every realm the report can change (the rooms it names
  -- and the ones the directory has on this shard), in key order; then the shards' counts
  PERFORM pg_advisory_xact_lock(7731030, sh);
  PERFORM verse.room_lock(
    (SELECT coalesce(array_agg(verse.room_lock_key(split_part(l, ' ', 2))), '{}') FROM unnest(coalesce(lines[first:], '{}')) l WHERE l ~ '^r [a-z0-9-]{1,24} ')
    || (SELECT coalesce(array_agg(hashtext(i.realm)) || array_agg(verse.room_lock_key(i.code)), '{}') FROM verse.room_instances i WHERE i.shard = sh));
  PERFORM pg_advisory_xact_lock(7731029, 0);
  -- holds long over whose room is gone (a realm's own are swept by its picks)
  DELETE FROM verse.room_holds h WHERE h.until_ms <= now - 60000 AND NOT EXISTS (SELECT 1 FROM verse.room_instances x WHERE x.code = h.code);
  IF g IS NOT NULL THEN
    SELECT s.gen INTO newest FROM verse.room_shards s WHERE s.shard = sh;
    IF newest IS NOT NULL AND g < newest THEN RETURN -2; END IF;   -- a replaced process: refused whole
  END IF;
  INSERT INTO verse.room_shards (shard, rooms_cap, free, at, gen, draining) VALUES (sh, 0, fr, at, g, CASE WHEN d THEN g END)
  ON CONFLICT (shard) DO UPDATE SET free = EXCLUDED.free, at = EXCLUDED.at,
    gen = coalesce(EXCLUDED.gen, verse.room_shards.gen),
    draining = CASE WHEN EXCLUDED.gen IS NULL THEN verse.room_shards.draining ELSE EXCLUDED.draining END
  WHERE verse.room_shards.at IS NULL OR verse.room_shards.at <= EXCLUDED.at OR EXCLUDED.gen > verse.room_shards.gen;
  FOREACH ln IN ARRAY coalesce(lines[first:], '{}') LOOP
    IF ln !~ '^r [a-z0-9-]{1,24} [0-9]{1,3}( [a-z0-9-]{1,40})*$' THEN RETURN -1; END IF;
    f := string_to_array(ln, ' ');
    named := named || f[2];
    INSERT INTO verse.room_instances AS x (code, realm, n, shard, players, pids, made_at, at)
    VALUES (f[2], coalesce(substring(f[2] FROM '^(.*)-[1-9][0-9]{0,2}$'), f[2]),
            substring(f[2] FROM '-([1-9][0-9]{0,2})$')::int, sh, f[3]::int, coalesce(f[4:], '{}'), now, at)
    ON CONFLICT (code) DO UPDATE SET shard = EXCLUDED.shard, players = EXCLUDED.players, pids = EXCLUDED.pids, at = EXCLUDED.at
    WHERE x.at IS NULL OR x.at <= EXCLUDED.at;
    DELETE FROM verse.room_holds h WHERE h.code = f[2] AND h.pid = ANY (f[4:]);
    m := m + 1;
  END LOOP;
  DELETE FROM verse.room_instances i WHERE i.shard = sh AND NOT (i.code = ANY (named))
    AND (i.at IS NOT NULL OR i.made_at <= now - 25000)
    AND NOT EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.until_ms > now);
  RETURN m;
END $_$;

CREATE OR REPLACE FUNCTION verse.room_invite(asked text, by_pid text, seats integer, OUT until_ms bigint, OUT why text) RETURNS record
    LANGUAGE plpgsql
    AS $$
DECLARE
  now bigint := verse.room_now();
  i verse.room_instances;
  tag text := 'i-' || by_pid;
  rl text;
BEGIN
  -- the instance's realm (kept for as long as the instance is) names the locks; then it is read again
  SELECT x.realm INTO rl FROM verse.room_instances x WHERE x.code = asked;
  PERFORM verse.room_lock(ARRAY[verse.room_lock_key(asked), hashtext(rl)]);
  IF rl IS NOT NULL THEN PERFORM verse.room_sweep_realm(rl, now); END IF;
  SELECT * INTO i FROM verse.room_instances x WHERE x.code = asked;
  IF NOT FOUND OR NOT (by_pid = ANY (i.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = asked AND h.pid = by_pid)) THEN
    why := 'not in it'; RETURN;
  END IF;
  DELETE FROM verse.room_holds h WHERE h.pid = tag AND h.code <> asked;
  IF NOT EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = asked AND h.pid = tag) AND verse.room_taken(i, now) >= seats THEN
    why := 'full'; RETURN;
  END IF;
  until_ms := now + verse.room_invite_ms();
  INSERT INTO verse.room_holds (code, pid, until_ms) VALUES (asked, tag, until_ms)
  ON CONFLICT ON CONSTRAINT room_holds_pkey DO UPDATE SET until_ms = EXCLUDED.until_ms;
END $$;

CREATE TABLE IF NOT EXISTS auth.limit_buckets (
  name text NOT NULL,
  key text NOT NULL CHECK (length(key) <= 80),
  fill_ms bigint NOT NULL,
  at_ms bigint NOT NULL,
  PRIMARY KEY (name, key)
);
CREATE INDEX IF NOT EXISTS limit_buckets_at ON auth.limit_buckets (at_ms);

-- One bucket's row, locked until the caller's transaction ends (made full when new); the caller
-- takes from it by its policy and writes it back with auth.bucket_put. now_ms is the database's clock.
CREATE OR REPLACE FUNCTION auth.bucket_lock(name_ text, key_ text, full_ms bigint, OUT fill_ms bigint, OUT at_ms bigint, OUT now_ms bigint)
LANGUAGE plpgsql AS $$
BEGIN
  now_ms := projects.now_ms();
  INSERT INTO auth.limit_buckets (name, key, fill_ms, at_ms) VALUES (name_, key_, full_ms, now_ms) ON CONFLICT DO NOTHING;
  SELECT b.fill_ms, b.at_ms INTO fill_ms, at_ms FROM auth.limit_buckets b WHERE b.name = name_ AND b.key = key_ FOR UPDATE;
  IF random() < 0.01 THEN DELETE FROM auth.limit_buckets b WHERE b.at_ms < now_ms - 86400000; END IF;
END $$;
CREATE OR REPLACE FUNCTION auth.bucket_put(name_ text, key_ text, fill_ms_ bigint, at_ms_ bigint) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.limit_buckets SET fill_ms = fill_ms_, at_ms = at_ms_ WHERE name = name_ AND key = key_ $$;
