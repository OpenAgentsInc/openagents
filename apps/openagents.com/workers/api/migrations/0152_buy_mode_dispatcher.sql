CREATE TABLE IF NOT EXISTS buy_mode_campaigns (
  campaign_id TEXT PRIMARY KEY,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL CHECK (state IN ('disabled', 'enabled', 'halted')),
  spend_enabled INTEGER NOT NULL CHECK (spend_enabled IN (0, 1)),
  per_job_cap_msats INTEGER NOT NULL CHECK (per_job_cap_msats > 0),
  daily_cap_msats INTEGER NOT NULL CHECK (daily_cap_msats > 0),
  spent_today_msats INTEGER NOT NULL DEFAULT 0 CHECK (spent_today_msats >= 0),
  day_key TEXT NOT NULL,
  operator_user_id TEXT NOT NULL,
  relay_url TEXT NOT NULL,
  last_alert_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS buy_mode_campaigns_updated_idx
  ON buy_mode_campaigns(updated_at DESC);

CREATE TABLE IF NOT EXISTS buy_mode_jobs (
  job_id TEXT PRIMARY KEY,
  campaign_id TEXT NOT NULL REFERENCES buy_mode_campaigns(campaign_id),
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  request_event_id TEXT NOT NULL UNIQUE,
  result_event_id TEXT UNIQUE,
  provider_pubkey TEXT,
  amount_msats INTEGER NOT NULL CHECK (amount_msats > 0),
  state TEXT NOT NULL CHECK (state IN ('issued', 'settled', 'settlement_blocked', 'settlement_failed')),
  receipt_ref TEXT,
  bolt11_ref TEXT,
  content_digest_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS buy_mode_jobs_campaign_updated_idx
  ON buy_mode_jobs(campaign_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS buy_mode_jobs_result_event_idx
  ON buy_mode_jobs(result_event_id)
  WHERE result_event_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS buy_mode_alerts (
  alert_id TEXT PRIMARY KEY,
  campaign_id TEXT NOT NULL REFERENCES buy_mode_campaigns(campaign_id),
  reason_ref TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS buy_mode_alerts_campaign_created_idx
  ON buy_mode_alerts(campaign_id, created_at DESC);
