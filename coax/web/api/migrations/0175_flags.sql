-- Flags (#13299, AAA P5.5; docs/deployment/release-set.md): a name and a whole number the API holds
-- for itself and the rooms, set with no deploy (POST /api/admin/flags) and read by every room on its
-- control poll two seconds later (coax/web/room/room-control.coax rm_flag). A flag may carry the time
-- it ends and a few words of why. What a name, a value and the words may be, when a flag still
-- holds and when "maintenance" refuses seats are coax/web/api/service-policy.coax's; this only keeps
-- the rows.
CREATE TABLE IF NOT EXISTS verse.flags (
  name text PRIMARY KEY CHECK (length(name) BETWEEN 1 AND 32),
  value bigint NOT NULL,
  until_ms bigint NOT NULL DEFAULT 0,      -- 0: until it is cleared
  why text NOT NULL DEFAULT '' CHECK (length(why) <= 160),
  set_ms bigint NOT NULL,
  set_by text NOT NULL DEFAULT ''
);

CREATE OR REPLACE FUNCTION verse.flag_put(name_ text, value_ bigint, until_ms_ bigint, why_ text, by_ text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO verse.flags (name, value, until_ms, why, set_ms, set_by) VALUES (name_, value_, until_ms_, why_, verse.now_ms(), by_)
  ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, until_ms = EXCLUDED.until_ms, why = EXCLUDED.why, set_ms = EXCLUDED.set_ms, set_by = EXCLUDED.set_by $$;

CREATE OR REPLACE FUNCTION verse.flag_clear(name_ text) RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM verse.flags WHERE name = name_ RETURNING 1) SELECT count(*)::int FROM d $$;
