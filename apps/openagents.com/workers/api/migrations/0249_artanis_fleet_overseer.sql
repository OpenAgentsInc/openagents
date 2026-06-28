-- Artanis fleet-overseer decision ledger (#6321).
-- Default-off scheduler scaffolding only: rows record watch/decide/proposed-act
-- outcomes, but they are not execution authority for stress, scale-out,
-- quarantine, spend, or fleet mutation.

CREATE TABLE IF NOT EXISTS artanis_fleet_overseer_decisions (
  id TEXT PRIMARY KEY,
  state TEXT NOT NULL CHECK (
    state IN (
      'reported',
      'autonomous_intent_recorded',
      'approval_requested',
      'no_action',
      'blocked',
      'skipped'
    )
  ),
  action_json TEXT NOT NULL,
  context_json TEXT NOT NULL,
  approval_gate_ref TEXT,
  health_snapshot_ref TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_fleet_overseer_decisions_created
  ON artanis_fleet_overseer_decisions (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_artanis_fleet_overseer_decisions_state_created
  ON artanis_fleet_overseer_decisions (state, created_at DESC);
