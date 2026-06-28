-- Artanis Pylon-support responder scheduled tick receipts (issue #6872).
-- One row is upserted per cron scheduled_at by the scan and compose stages.
-- The row is public-safe operational evidence only: bounded counts, states,
-- timestamps, and a stable tick ref. It records no prompt text, reply body,
-- provider payload, wallet material, or private user data.

CREATE TABLE IF NOT EXISTS artanis_responder_ticks (
  tick_ref TEXT PRIMARY KEY,
  scheduled_at TEXT NOT NULL UNIQUE,
  scan_state TEXT NOT NULL DEFAULT 'pending' CHECK (
    scan_state IN ('pending', 'ran', 'skipped', 'error')
  ),
  scan_scanned INTEGER NOT NULL DEFAULT 0,
  scan_proposed INTEGER NOT NULL DEFAULT 0,
  scan_blocked INTEGER NOT NULL DEFAULT 0,
  scan_skipped INTEGER NOT NULL DEFAULT 0,
  scan_skipped_reason TEXT,
  compose_state TEXT NOT NULL DEFAULT 'pending' CHECK (
    compose_state IN ('pending', 'ran', 'skipped', 'error')
  ),
  compose_considered INTEGER NOT NULL DEFAULT 0,
  compose_responded INTEGER NOT NULL DEFAULT 0,
  compose_blocked INTEGER NOT NULL DEFAULT 0,
  compose_tipped INTEGER NOT NULL DEFAULT 0,
  compose_skipped_reason TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_artanis_responder_ticks_scheduled_at
  ON artanis_responder_ticks (scheduled_at DESC);
