-- Capacity records (#13296, AAA P5.2; docs/deployment/soak.md): one night's soak of the full fleet,
-- the bytes capacity record v1 says (coax/web/api/soak-record.coax writes and checks them; the API
-- checks a posted record through that codec before this is called). One row a day and environment:
-- a later run of the same day replaces the earlier. The status page shows the newest.
CREATE TABLE IF NOT EXISTS verse.capacity_records (
  day date NOT NULL,
  env text NOT NULL CHECK (env IN ('staging', 'production', 'local')),
  at_ms bigint NOT NULL,           -- when the run ended (the record's t)
  seated int NOT NULL,             -- the most seated at once: the capacity the night held
  holds boolean NOT NULL,          -- the verdict
  body text NOT NULL CHECK (length(body) <= 2048),
  PRIMARY KEY (day, env)
);

-- Keeps a record: true when it is now the day's, false when a later run of that day is already kept.
CREATE OR REPLACE FUNCTION verse.capacity_put(day_ date, env_ text, at_ms_ bigint, seated_ int, holds_ boolean, body_ text) RETURNS boolean
LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  INSERT INTO verse.capacity_records AS c (day, env, at_ms, seated, holds, body) VALUES (day_, env_, at_ms_, seated_, holds_, body_)
  ON CONFLICT (day, env) DO UPDATE SET at_ms = EXCLUDED.at_ms, seated = EXCLUDED.seated, holds = EXCLUDED.holds, body = EXCLUDED.body
  WHERE c.at_ms <= EXCLUDED.at_ms;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n = 1;
END $$;

-- The newest record's body (staging's first: the nightly soak plays staging), or one day's; NULL none.
CREATE OR REPLACE FUNCTION verse.capacity_record(day_ date) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT c.body FROM verse.capacity_records c
  WHERE c.env <> 'local' AND (day_ IS NULL OR c.day = day_)
  ORDER BY c.day DESC, (c.env = 'staging') DESC, c.at_ms DESC LIMIT 1 $$;
