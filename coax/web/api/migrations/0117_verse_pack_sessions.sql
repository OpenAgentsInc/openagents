-- Private human packs for every visitor (verse_packs.c, docs/verse/metahuman-support.md): one row a
-- session. A session is asked for (POST /api/verse/packs/<name>) and fetched once, within its
-- seconds (GET /api/verse/packs/fetch/<sid>). The address is kept only as its SHA-256, for the
-- hourly limit; rows go after two days.
CREATE TABLE IF NOT EXISTS verse.pack_sessions (
  sid text PRIMARY KEY,
  at timestamptz NOT NULL DEFAULT now(),
  ip_sha256 text NOT NULL,
  pack text NOT NULL,
  bytes bigint NOT NULL,
  fetched_at timestamptz
);
CREATE INDEX IF NOT EXISTS pack_sessions_ip ON verse.pack_sessions (ip_sha256, at);
CREATE INDEX IF NOT EXISTS pack_sessions_fetched ON verse.pack_sessions (fetched_at) WHERE fetched_at IS NOT NULL;

-- A new session for an address: 'ok', 'hour' (per_hour asked for in the last hour) or 'day' (the
-- UTC day's fetched bytes and this one over daily_bytes).
CREATE OR REPLACE FUNCTION verse.pack_issue(sid text, ip text, pack text, bytes bigint, per_hour int, daily_bytes bigint)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE h text := encode(sha256(convert_to(ip, 'UTF8')), 'hex');
BEGIN
  DELETE FROM verse.pack_sessions WHERE at < now() - interval '2 days';
  IF (SELECT count(*) FROM verse.pack_sessions s WHERE s.ip_sha256 = h AND s.at > now() - interval '1 hour') >= per_hour THEN
    RETURN 'hour';
  END IF;
  IF (SELECT coalesce(sum(s.bytes), 0) FROM verse.pack_sessions s
      WHERE s.fetched_at >= date_trunc('day', now() AT TIME ZONE 'UTC') AT TIME ZONE 'UTC') + bytes > daily_bytes THEN
    RETURN 'day';
  END IF;
  INSERT INTO verse.pack_sessions (sid, ip_sha256, pack, bytes) VALUES (sid, h, pack, bytes);
  RETURN 'ok';
END $$;

-- A session's pack, once: NULL when it is unknown, used, or older than secs.
CREATE OR REPLACE FUNCTION verse.pack_fetch(id text, secs int) RETURNS text LANGUAGE sql AS $$
  UPDATE verse.pack_sessions SET fetched_at = now()
  WHERE sid = id AND fetched_at IS NULL AND at > now() - make_interval(secs => secs)
  RETURNING pack
$$;
