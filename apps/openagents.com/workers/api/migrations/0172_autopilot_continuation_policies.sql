-- M6 (#4764): user-settable auto-continuation policy and continuation
-- attempt ledger. The policy converts the operator-only continue API into
-- product behavior under budget gates and a max-continuations counter.
CREATE TABLE IF NOT EXISTS autopilot_continuation_policies (
  user_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 0,
  max_continuations_per_run INTEGER NOT NULL DEFAULT 2,
  max_continuations_per_day INTEGER NOT NULL DEFAULT 10,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS autopilot_continuation_events (
  id TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  run_id TEXT NOT NULL,
  goal_id TEXT,
  mode TEXT NOT NULL CHECK (mode IN ('follow_up_turn', 'goal_continuation')),
  decision TEXT NOT NULL CHECK (decision IN ('dispatched', 'failed', 'skipped')),
  reason_ref TEXT NOT NULL,
  attempt INTEGER NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE (run_id, attempt)
);

CREATE INDEX IF NOT EXISTS idx_autopilot_continuation_events_user_created
  ON autopilot_continuation_events (user_id, created_at);

CREATE INDEX IF NOT EXISTS idx_autopilot_continuation_events_run
  ON autopilot_continuation_events (run_id);
