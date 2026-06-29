ALTER TABLE forge_promotion_decisions
  ADD COLUMN target_ref TEXT NOT NULL DEFAULT '';

ALTER TABLE forge_promotion_decisions
  ADD COLUMN queue_position INTEGER NOT NULL DEFAULT 0;

ALTER TABLE forge_promotion_decisions
  ADD COLUMN gate_results_json TEXT NOT NULL DEFAULT '[]';
