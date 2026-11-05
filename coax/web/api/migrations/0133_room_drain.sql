-- Draining shards and process generations in the instance directory (#12797).
--
-- A room process names its generation (its start, wall ms: the vitals' operation IDs use the same) in
-- its report, and d while it drains before a deploy (coax/web/room/room-directory.coax):
--   t MS / s SHARD FREE / g GEN [d] / r CODE PLAYERS PID ... per room
-- room_shards keeps the newest generation that reported for each shard (gen) and whether it drains
-- (draining: that generation, NULL none). A report from an older generation than the shard's is
-- refused whole (-2, the API's 409): during a revision change the old process and the new both report,
-- and only the new one's rooms are the shard's, whatever wall time the old one's report carries. A
-- report with no g line (a room from before) is kept as before.
--
-- A draining shard takes nobody: verse.room_fits is false for it, so room_pick places neither a new
-- instance, a reconnect, an asked instance nor the lowest public one there, while the generation that
-- drains is the newest and reported in the last 60 s (a drained process that stopped reporting and a
-- replacement that never came frees the shard after a minute; the new process's first report clears it).
-- Additive, and safe to run twice.
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS gen bigint;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS draining bigint;

CREATE OR REPLACE FUNCTION verse.room_draining(k int, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.room_shards s
                 WHERE s.shard = k AND s.draining IS NOT NULL AND s.draining = s.gen AND coalesce(s.at, 0) > now - 60000) $$;

-- room_fits (0128) and not draining.
CREATE OR REPLACE FUNCTION verse.room_fits(k int, bld text, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT NOT verse.room_draining(k, now) AND (coalesce(bld, '') = '' OR NOT EXISTS (
    SELECT 1 FROM verse.room_shards s WHERE s.shard = k AND s.build IS NOT NULL AND s.build_at > now - 60000 AND s.build <> bld)) $$;

-- room_report (0115) with the generation line.
CREATE OR REPLACE FUNCTION verse.room_report(body text) RETURNS int LANGUAGE plpgsql AS $$
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
  PERFORM pg_advisory_xact_lock(7731028);
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
END $$;
