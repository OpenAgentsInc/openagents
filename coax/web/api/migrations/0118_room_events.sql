-- Everglade's world events and levels 1 to 3 (epic #12737, #12738, #12739; spec
-- docs/verse/2026-11-03-everglade-world-events.md sections 8 and 10).
--   - verse.room_events: a realm's event dials, one row a realm, that its admins set from the events
--     pane (POST /api/playtest/room-event). oa-room hears them in room-control's "event" and
--     "levels" lines. start_n and stop_n count START NOW and STOP presses, so a shard acts on each
--     press once. Off until an admin switches it on.
--   - the levels switch and its cap (level_cap, 1 to 3, 2 for now: one change raises it), and a
--     seat ticket's ACCT 2 for the realm's admins, who may play Everglade's own demo character.
--   - verse.room_vitals.xp: a player's XP, which oa-room reports with their HP and the seat ticket
--     brings back (its HP field as HPxXP). It only rises.
-- Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_events (
  slug text PRIMARY KEY,
  dials jsonb NOT NULL DEFAULT '{}',
  start_n int NOT NULL DEFAULT 0,
  stop_n int NOT NULL DEFAULT 0,
  started_ms bigint,
  ms bigint,
  by_who text
);
INSERT INTO verse.room_events (slug, dials, ms) VALUES ('everglade', '{}'::jsonb, verse.now_ms()) ON CONFLICT (slug) DO NOTHING;
ALTER TABLE verse.room_vitals ADD COLUMN IF NOT EXISTS xp int;

-- The dials as oa-room and the pane read them, each clamped to its range (spec 8.2); a dial not
-- set takes its default.
CREATE OR REPLACE FUNCTION verse.room_event_dials(j jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  WITH n AS (SELECT
    CASE WHEN jsonb_typeof(j->'on') = 'boolean' THEN (j->>'on')::boolean ELSE false END AS on_,
    CASE WHEN jsonb_typeof(j->'every') = 'number' THEN (j->>'every')::numeric::int ELSE 0 END AS every,
    CASE WHEN jsonb_typeof(j->'waves') = 'number' THEN (j->>'waves')::numeric::int ELSE 3 END AS waves,
    CASE WHEN jsonb_typeof(j->'size') = 'number' THEN (j->>'size')::numeric::int ELSE 12 END AS size,
    CASE WHEN jsonb_typeof(j->'cultists') = 'number' THEN (j->>'cultists')::numeric::int ELSE 6 END AS cult,
    CASE WHEN jsonb_typeof(j->'fanatics') = 'number' THEN (j->>'fanatics')::numeric::int ELSE 1 END AS fan,
    CASE WHEN jsonb_typeof(j->'warlocks') = 'number' THEN (j->>'warlocks')::numeric::int ELSE 3 END AS wl,
    CASE WHEN jsonb_typeof(j->'warlock_level') = 'number' THEN (j->>'warlock_level')::numeric::int ELSE 3 END AS wlmax,
    CASE WHEN jsonb_typeof(j->'gates') = 'number' THEN (j->>'gates')::numeric::int ELSE 31 END AS gates,
    CASE WHEN jsonb_typeof(j->'leader') = 'boolean' THEN (j->>'leader')::boolean ELSE true END AS arch,
    CASE WHEN jsonb_typeof(j->'aggression') = 'number' THEN (j->>'aggression')::numeric::int ELSE 2 END AS aggr,
    CASE WHEN jsonb_typeof(j->'town_damage') = 'boolean' THEN (j->>'town_damage')::boolean ELSE true END AS harm,
    CASE WHEN jsonb_typeof(j->'reward') = 'number' THEN (j->>'reward')::numeric::int ELSE 100 END AS reward,
    CASE WHEN jsonb_typeof(j->'minutes') = 'number' THEN (j->>'minutes')::numeric::int ELSE 15 END AS minutes,
    CASE WHEN jsonb_typeof(j->'levels') = 'boolean' THEN (j->>'levels')::boolean ELSE false END AS levels,
    CASE WHEN jsonb_typeof(j->'level_cap') = 'number' THEN (j->>'level_cap')::numeric::int ELSE 2 END AS cap)
  SELECT jsonb_build_object('on', on_, 'every', CASE WHEN every <= 0 THEN 0 ELSE least(greatest(every, 15), 240) END,
    'waves', least(greatest(waves, 1), 8), 'size', least(greatest(size, 4), 24),
    'cultists', least(greatest(cult, 0), 10), 'fanatics', least(greatest(fan, 0), 10), 'warlocks', least(greatest(wl, 0), 10),
    'warlock_level', least(greatest(wlmax, 1), 3), 'gates', least(greatest(gates, 1), 31),
    'leader', arch, 'aggression', least(greatest(aggr, 0), 3), 'town_damage', harm,
    'reward', least(greatest(reward, 50), 300), 'minutes', least(greatest(minutes, 5), 30), 'levels', levels,
    'level_cap', least(greatest(cap, 1), 3)) FROM n $$;
