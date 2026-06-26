-- Published Gym benchmark LADDER snapshots (#6309, epic #6303 Khala GTM).
--
-- One row per ladder, keyed by the public-safe `ladder_ref`. Each row stores the
-- already-public-safe `openagents.gym.ladder_leaderboard.v1` object (built via
-- buildGymLadderLeaderboard, which derives every rung from the shipped
-- buildGymLeaderboardProjection — decision-grade + public-safety-checked rows
-- only) as a JSON blob, plus publish freshness. There is NO raw benchmark content
-- here: prompts, responses, logs, trajectories, keys, and private endpoints are
-- dropped by the flat projection's public-safety boundary and never reach this
-- table. The recurring publisher upserts by ladder_ref each cadence (per model
-- release / weekly / on demand). The public `/api/public/gym/leaderboard` route
-- reads the latest snapshot; when none exists it serves the honest empty ladder
-- shape (all rungs awaiting_owner with their owner-gate refs).
CREATE TABLE IF NOT EXISTS gym_ladder_leaderboard_snapshots (
  ladder_ref TEXT PRIMARY KEY,
  ladder_json TEXT NOT NULL,
  published_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_gym_ladder_leaderboard_snapshots_published_at
  ON gym_ladder_leaderboard_snapshots (published_at DESC, ladder_ref ASC);
