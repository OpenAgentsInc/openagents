-- Pluralis roadmap P0.2 (openagents#4849, forum post 6197bd1b, rail #4748/#4673):
-- window-seal records carry the staleness distribution of merged
-- contributions, contributor-churn events, and verification overhead as a
-- fraction of window cost; the run authority carries max_allowed_stale as a
-- per-run contract value (Pluralis node0 ships 5 as prior art).

ALTER TABLE training_runs
  ADD COLUMN max_allowed_stale INTEGER NOT NULL DEFAULT 5;

ALTER TABLE training_windows
  ADD COLUMN seal_metadata_json TEXT;
