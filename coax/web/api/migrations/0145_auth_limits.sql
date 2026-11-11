-- Rate limits, shared by every instance of the API (docs/auth/sessions-and-tokens.md#rate-limits,
-- #13203). The limits themselves (how many, how long, keyed by what, and the words shown) are one
-- table in auth.c, which /docs/limits and GET /api/auth/limits are made from; whether a hit is
-- allowed and when a full window lifts is decided in auth-policy.coax. This table only counts:
-- one row per limit and key, the window's start and its count. A key is a digest, never an
-- address or an email.
CREATE TABLE auth.limit_counts (
  name text NOT NULL,
  key text NOT NULL CHECK (length(key) <= 80),
  started_ms bigint NOT NULL,
  n int NOT NULL,
  PRIMARY KEY (name, key)
);
CREATE INDEX limit_counts_started ON auth.limit_counts (started_ms);

-- The window's facts for one hit, counted only when counted is true: {started_ms, n, now_ms}.
-- The caller (auth-policy.coax) decides; a window older than window_ms starts again here.
CREATE FUNCTION auth.limit_hit(name_ text, key_ text, window_ms bigint, count_it boolean,
                               OUT started_ms bigint, OUT n int, OUT now_ms bigint) LANGUAGE plpgsql AS $$
BEGIN
  now_ms := projects.now_ms();
  INSERT INTO auth.limit_counts AS c (name, key, started_ms, n) VALUES (name_, key_, now_ms, CASE WHEN count_it THEN 1 ELSE 0 END)
  ON CONFLICT (name, key) DO UPDATE SET
    started_ms = CASE WHEN c.started_ms <= now_ms - window_ms THEN now_ms ELSE c.started_ms END,
    n = CASE WHEN c.started_ms <= now_ms - window_ms THEN 0 ELSE c.n END + CASE WHEN count_it THEN 1 ELSE 0 END
  RETURNING c.started_ms, c.n INTO started_ms, n;
END $$;

-- Old windows go (the cleanup job, and now and then on a hit).
CREATE FUNCTION auth.limit_sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.limit_counts WHERE started_ms < projects.now_ms() - 8 * 86400000::bigint RETURNING 1) SELECT count(*)::int FROM d
$$;
