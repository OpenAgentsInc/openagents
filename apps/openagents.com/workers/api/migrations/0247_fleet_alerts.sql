-- 0247_fleet_alerts.sql (#6408)
--
-- Durable record surface for the "fleet never silently stalls" watchdog. The
-- 1-minute Worker cron (runFleetBurnStallDetectorScheduled) classifies the live
-- own-capacity Codex burn rate against active coding leases and, on a stall,
-- writes a loud row here plus any auto-recovery actions it took. This is the
-- Cloudflare-native alert/audit sink (no third-party notification surface), per
-- the prefer-cloudflare-primitives invariant.

CREATE TABLE IF NOT EXISTS fleet_alerts (
  id TEXT PRIMARY KEY,
  alert_ref TEXT NOT NULL UNIQUE,
  detected_at TEXT NOT NULL,
  -- Classification persisted for the burn window. Only 'stalled' rows are
  -- written; healthy / idle_no_work ticks are not persisted (no false alarms).
  classification TEXT NOT NULL,
  reason_ref TEXT NOT NULL,
  burn_tokens_window INTEGER NOT NULL,
  window_minutes INTEGER NOT NULL,
  stall_threshold_tokens INTEGER NOT NULL,
  active_assignments INTEGER NOT NULL,
  queued_assignments INTEGER NOT NULL,
  -- JSON array of recovery actions taken on this tick (flushed lease refs,
  -- counts, owner pylon refs, skips). Public-safe refs only.
  recovery_actions_json TEXT NOT NULL,
  recovered_lease_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_fleet_alerts_detected_at
  ON fleet_alerts(detected_at DESC);

CREATE INDEX IF NOT EXISTS idx_fleet_alerts_classification_detected
  ON fleet_alerts(classification, detected_at DESC);
