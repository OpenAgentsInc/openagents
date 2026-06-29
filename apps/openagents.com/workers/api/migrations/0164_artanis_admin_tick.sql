-- Artanis administrator tick decisions (issue #4701): every
-- model-decided tick action is a row - dispatched, no_action, blocked
-- (schema-invalid mind output with the raw proposal), or
-- dispatch_failed. Nothing silent.

CREATE TABLE IF NOT EXISTS artanis_admin_tick_decisions (
  id TEXT PRIMARY KEY,
  state TEXT NOT NULL CHECK (
    state IN ('dispatched', 'no_action', 'blocked', 'dispatch_failed')
  ),
  action_json TEXT NOT NULL DEFAULT '{}',
  assignment_ref TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_admin_tick_decisions_day
  ON artanis_admin_tick_decisions (state, created_at DESC);
