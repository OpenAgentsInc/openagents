-- Operator workspace seeding invite and engagement tracking (Epic C / #5093).
--
-- These fields are public-safe workflow telemetry only. They do not grant
-- workspace authority, spend, payout, settlement, provider, or delivery rights.

ALTER TABLE prefilled_workspaces
  ADD COLUMN invited_at TEXT;

ALTER TABLE prefilled_workspaces
  ADD COLUMN first_viewed_at TEXT;

ALTER TABLE prefilled_workspaces
  ADD COLUMN first_claimed_at TEXT;

ALTER TABLE prefilled_workspaces
  ADD COLUMN first_run_at TEXT;

ALTER TABLE prefilled_workspaces
  ADD COLUMN last_viewed_at TEXT;

ALTER TABLE prefilled_workspaces
  ADD COLUMN revisit_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS prefilled_workspaces_engagement_idx
  ON prefilled_workspaces(status, first_claimed_at, first_run_at, updated_at DESC)
  WHERE archived_at IS NULL;
