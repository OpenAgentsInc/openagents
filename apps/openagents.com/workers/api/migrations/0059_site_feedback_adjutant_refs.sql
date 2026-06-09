ALTER TABLE site_revision_feedback
  ADD COLUMN adjutant_assignment_id TEXT REFERENCES adjutant_assignments(id) ON DELETE SET NULL;

ALTER TABLE site_revision_feedback
  ADD COLUMN adjutant_adjustment_id TEXT REFERENCES adjutant_adjustment_requests(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS site_revision_feedback_adjutant_assignment_idx
  ON site_revision_feedback(adjutant_assignment_id)
  WHERE adjutant_assignment_id IS NOT NULL
    AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS site_revision_feedback_adjutant_adjustment_idx
  ON site_revision_feedback(adjutant_adjustment_id)
  WHERE adjutant_adjustment_id IS NOT NULL
    AND archived_at IS NULL;
