CREATE TABLE IF NOT EXISTS email_campaigns (
  id TEXT PRIMARY KEY NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  audience TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('draft', 'active', 'paused', 'archived')),
  source_authority_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS email_campaigns_status_updated_idx
  ON email_campaigns(status, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_campaign_steps (
  id TEXT PRIMARY KEY NOT NULL,
  campaign_id TEXT NOT NULL REFERENCES email_campaigns(id) ON DELETE CASCADE,
  step_key TEXT NOT NULL,
  name TEXT NOT NULL,
  delay_seconds INTEGER NOT NULL CHECK (delay_seconds >= 0),
  template_slug TEXT NOT NULL,
  lifecycle_kind TEXT,
  status TEXT NOT NULL CHECK (status IN ('draft', 'active', 'paused', 'archived')),
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE(campaign_id, step_key)
);

CREATE INDEX IF NOT EXISTS email_campaign_steps_campaign_delay_idx
  ON email_campaign_steps(campaign_id, delay_seconds ASC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS email_campaign_enrollments (
  id TEXT PRIMARY KEY NOT NULL,
  campaign_id TEXT NOT NULL REFERENCES email_campaigns(id) ON DELETE CASCADE,
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  email TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'completed', 'suppressed', 'canceled')
  ),
  idempotency_key TEXT NOT NULL UNIQUE,
  source_authority_ref TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  enrolled_at TEXT NOT NULL,
  completed_at TEXT,
  canceled_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS email_campaign_enrollments_campaign_status_idx
  ON email_campaign_enrollments(campaign_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS email_campaign_enrollments_email_idx
  ON email_campaign_enrollments(email, updated_at DESC);

CREATE TABLE IF NOT EXISTS email_campaign_sends (
  id TEXT PRIMARY KEY NOT NULL,
  campaign_id TEXT NOT NULL REFERENCES email_campaigns(id) ON DELETE CASCADE,
  step_id TEXT NOT NULL REFERENCES email_campaign_steps(id) ON DELETE CASCADE,
  enrollment_id TEXT NOT NULL REFERENCES email_campaign_enrollments(id) ON DELETE CASCADE,
  user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  email TEXT NOT NULL,
  due_at TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('scheduled', 'claimed', 'sent', 'skipped', 'suppressed', 'failed', 'canceled')
  ),
  idempotency_key TEXT NOT NULL UNIQUE,
  source_authority_ref TEXT NOT NULL,
  email_message_id TEXT REFERENCES email_messages(id) ON DELETE SET NULL,
  provider_event_id TEXT,
  error_name TEXT,
  error_message TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  claimed_at TEXT,
  sent_at TEXT,
  skipped_at TEXT,
  failed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS email_campaign_sends_due_idx
  ON email_campaign_sends(status, due_at ASC);

CREATE INDEX IF NOT EXISTS email_campaign_sends_enrollment_idx
  ON email_campaign_sends(enrollment_id, due_at ASC);

CREATE TABLE IF NOT EXISTS email_suppression_entries (
  id TEXT PRIMARY KEY NOT NULL,
  email TEXT NOT NULL,
  reason TEXT NOT NULL CHECK (
    reason IN ('unsubscribe', 'bounce', 'complaint', 'operator', 'manual')
  ),
  scope TEXT NOT NULL CHECK (scope IN ('marketing', 'drip', 'all')),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
  source_authority_ref TEXT NOT NULL,
  provider_event_id TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS email_suppression_entries_email_active_idx
  ON email_suppression_entries(email, active, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS email_preferences (
  id TEXT PRIMARY KEY NOT NULL,
  user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
  email TEXT NOT NULL,
  marketing_opt_in INTEGER NOT NULL DEFAULT 1 CHECK (marketing_opt_in IN (0, 1)),
  drip_opt_in INTEGER NOT NULL DEFAULT 1 CHECK (drip_opt_in IN (0, 1)),
  transactional_opt_in INTEGER NOT NULL DEFAULT 1 CHECK (transactional_opt_in IN (0, 1)),
  source_authority_ref TEXT NOT NULL,
  updated_by_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(email)
);

CREATE INDEX IF NOT EXISTS email_preferences_user_idx
  ON email_preferences(user_id, updated_at DESC)
  WHERE user_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS email_provider_events (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL CHECK (provider IN ('resend', 'gmail')),
  provider_event_id TEXT NOT NULL,
  event_type TEXT NOT NULL,
  email TEXT,
  email_message_id TEXT REFERENCES email_messages(id) ON DELETE SET NULL,
  provider_message_id TEXT,
  occurred_at TEXT,
  payload_summary_json TEXT NOT NULL DEFAULT '{}',
  source_authority_ref TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(provider, provider_event_id)
);

CREATE INDEX IF NOT EXISTS email_provider_events_type_created_idx
  ON email_provider_events(provider, event_type, created_at DESC);
