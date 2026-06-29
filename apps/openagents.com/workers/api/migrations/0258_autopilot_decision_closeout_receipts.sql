CREATE TABLE IF NOT EXISTS autopilot_decision_closeout_receipts (
  closeout_ref TEXT PRIMARY KEY,
  decision_ref TEXT NOT NULL,
  work_order_ref TEXT NOT NULL,
  action TEXT NOT NULL,
  resolved_state TEXT NOT NULL,
  outcome TEXT NOT NULL,
  actor_agent_user_id TEXT NOT NULL,
  decided_at TEXT NOT NULL,
  receipt_refs_json TEXT NOT NULL,
  has_answer INTEGER NOT NULL DEFAULT 0,
  line TEXT NOT NULL,
  receipt_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_autopilot_decision_closeout_receipts_work
  ON autopilot_decision_closeout_receipts(work_order_ref, decided_at DESC);
