-- 0248_pylon_executor_quarantines.sql (#6424)
--
-- Cloudflare-native kill-switch ledger for Pylon executors. Operator control
-- writes an active quarantine row here; Worker routes read it before exposing
-- heartbeat, assignment polling, or dispatch surfaces to the executor.

CREATE TABLE IF NOT EXISTS pylon_executor_quarantines (
  id TEXT PRIMARY KEY,
  quarantine_ref TEXT NOT NULL UNIQUE,
  pylon_ref TEXT NOT NULL,
  operator_agent_user_id TEXT NOT NULL,
  status TEXT NOT NULL,
  reason_refs_json TEXT NOT NULL,
  source_refs_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pylon_executor_quarantines_pylon_status_created
  ON pylon_executor_quarantines(pylon_ref, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_pylon_executor_quarantines_created
  ON pylon_executor_quarantines(created_at DESC);
