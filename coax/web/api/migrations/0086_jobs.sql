-- Scheduled jobs (jobs.c, svc-schedules): one run per job per Chicago day.
CREATE SCHEMA IF NOT EXISTS jobs;
CREATE TABLE IF NOT EXISTS jobs.runs (
  job text NOT NULL,
  day date NOT NULL,
  state text NOT NULL CHECK (state IN ('running', 'ran', 'failed')),
  runs int NOT NULL DEFAULT 1,
  result jsonb,
  started_at timestamptz NOT NULL DEFAULT now(),
  finished_at timestamptz,
  PRIMARY KEY (job, day)
);

-- The day's claim for a job: {state: 'run', day} to run it now; {state:
-- 'already ran', day, result}; or {state: 'running', day} while another call
-- runs it (for ten minutes). The day is today in Chicago unless given; again
-- runs a day that already ran.
CREATE OR REPLACE FUNCTION jobs.claim(j text, given text, again boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  d date := coalesce(given::date, (now() AT TIME ZONE 'America/Chicago')::date);
  r jobs.runs;
BEGIN
  INSERT INTO jobs.runs (job, day, state) VALUES (j, d, 'running') ON CONFLICT DO NOTHING;
  IF FOUND THEN RETURN jsonb_build_object('state', 'run', 'day', d); END IF;
  SELECT * INTO r FROM jobs.runs WHERE job = j AND day = d FOR UPDATE;
  IF r.state = 'ran' AND NOT again THEN RETURN jsonb_build_object('state', 'already ran', 'day', d, 'result', r.result); END IF;
  IF r.state = 'running' AND r.started_at > now() - interval '10 minutes' THEN RETURN jsonb_build_object('state', 'running', 'day', d); END IF;
  UPDATE jobs.runs SET state = 'running', runs = runs + 1, started_at = now(), finished_at = NULL WHERE job = j AND day = d;
  RETURN jsonb_build_object('state', 'run', 'day', d);
END $$;

CREATE OR REPLACE FUNCTION jobs.finish(j text, d date, res jsonb) RETURNS void LANGUAGE sql AS $$
  UPDATE jobs.runs SET state = CASE WHEN res IS NULL THEN 'failed' ELSE 'ran' END, result = res, finished_at = now()
   WHERE job = j AND day = d
$$;
