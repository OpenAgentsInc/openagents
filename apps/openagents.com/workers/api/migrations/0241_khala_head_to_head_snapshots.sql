-- Published Khala external HEAD-TO-HEAD snapshots (#6308, epic #6303 Khala GTM).
--
-- One row per head-to-head, keyed by the public-safe `head_to_head_ref`. Each row
-- stores the already-public-safe `openagents.khala.head_to_head.v1` object (built
-- via buildKhalaHeadToHead, which derives every matchup from the shipped
-- buildGymLeaderboardProjection — decision-grade + public-safety-checked rows
-- only) as a JSON blob, plus publish freshness. There is NO raw benchmark content
-- here: prompts, responses, logs, trajectories, keys, and private endpoints are
-- dropped by the flat projection's public-safety boundary and never reach this
-- table. The recurring publisher upserts by head_to_head_ref each cadence (per
-- Khala release / weekly / on demand). The public
-- `/api/public/khala/head-to-head` route reads the latest snapshot; when none
-- exists it serves the honest empty shape (all matchups awaiting_owner with their
-- owner-gate refs).
CREATE TABLE IF NOT EXISTS khala_head_to_head_snapshots (
  head_to_head_ref TEXT PRIMARY KEY,
  head_to_head_json TEXT NOT NULL,
  published_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_head_to_head_snapshots_published_at
  ON khala_head_to_head_snapshots (published_at DESC, head_to_head_ref ASC);
