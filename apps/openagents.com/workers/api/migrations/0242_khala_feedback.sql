-- Khala CLI feedback submissions.
--
-- Public clients submit bounded text plus an optional trace_ref; operator reads
-- stay admin-token gated in the Worker.

CREATE TABLE IF NOT EXISTS khala_feedback (
  feedback_ref TEXT PRIMARY KEY,
  trace_ref TEXT,
  feedback_text TEXT NOT NULL,
  source TEXT NOT NULL,
  client_version TEXT,
  user_agent TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_feedback_created_at
  ON khala_feedback(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_khala_feedback_trace_ref
  ON khala_feedback(trace_ref, created_at DESC);

