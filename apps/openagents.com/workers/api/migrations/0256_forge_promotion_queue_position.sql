-- FORGE SU-4 (#6794): persist deterministic virtual-merge-queue position on
-- promotion decision receipts.

ALTER TABLE forge_promotion_decisions
  ADD COLUMN queue_position INTEGER NOT NULL DEFAULT 0 CHECK (queue_position >= 0);
