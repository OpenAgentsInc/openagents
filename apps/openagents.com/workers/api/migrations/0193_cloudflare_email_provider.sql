PRAGMA foreign_keys = OFF;

DROP TABLE IF EXISTS email_messages_0193_new;
DROP TABLE IF EXISTS email_deliveries_0193_new;
DROP TABLE IF EXISTS email_drafts_0193_new;
DROP TABLE IF EXISTS email_provider_events_0193_new;

CREATE TABLE email_messages_0193_new (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL CHECK (
    kind IN (
      'prelaunch_invitation',
      'billing_out_of_credits',
      'operator_notification',
      'crm_transactional'
    )
  ),
  actor_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  target_user_id TEXT REFERENCES users(id) ON DELETE SET NULL,
  to_email TEXT NOT NULL,
  from_email TEXT NOT NULL,
  reply_to_email TEXT,
  subject TEXT NOT NULL,
  text_body TEXT NOT NULL,
  html_body TEXT NOT NULL,
  template_id TEXT REFERENCES email_templates(id) ON DELETE SET NULL,
  template_slug TEXT NOT NULL,
  template_context_json TEXT NOT NULL DEFAULT '{}',
  status TEXT NOT NULL CHECK (
    status IN (
      'reserved',
      'rendered',
      'accepted',
      'failed',
      'draft_recorded'
    )
  ),
  provider TEXT CHECK (
    provider IS NULL
    OR provider IN ('resend', 'gmail', 'cloudflare_email')
  ),
  provider_message_id TEXT,
  provider_draft_id TEXT,
  provider_thread_id TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  source_authority_ref TEXT NOT NULL,
  action_submission_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  error_name TEXT,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO email_messages_0193_new
SELECT * FROM email_messages;

CREATE TABLE email_deliveries_0193_new (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES email_messages(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (
    provider IN ('resend', 'gmail', 'cloudflare_email')
  ),
  provider_message_id TEXT,
  provider_thread_id TEXT,
  provider_request_id TEXT,
  provider_idempotency_key TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'accepted', 'failed', 'unknown_external_state')
  ),
  error_name TEXT,
  error_message TEXT,
  provider_payload_summary_json TEXT NOT NULL DEFAULT '{}',
  attempted_at TEXT NOT NULL,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

INSERT INTO email_deliveries_0193_new
SELECT * FROM email_deliveries;

CREATE TABLE email_drafts_0193_new (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL REFERENCES email_messages(id) ON DELETE CASCADE,
  provider TEXT NOT NULL CHECK (
    provider IN ('resend', 'gmail', 'cloudflare_email')
  ),
  provider_draft_id TEXT NOT NULL,
  provider_message_id TEXT,
  provider_thread_id TEXT,
  status TEXT NOT NULL CHECK (
    status IN (
      'draft_requested',
      'draft_created',
      'draft_failed',
      'sent_from_draft',
      'abandoned'
    )
  ),
  provenance_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  sent_at TEXT,
  UNIQUE(provider, provider_draft_id)
);

INSERT INTO email_drafts_0193_new
SELECT * FROM email_drafts;

CREATE TABLE email_provider_events_0193_new (
  id TEXT PRIMARY KEY NOT NULL,
  provider TEXT NOT NULL CHECK (
    provider IN ('resend', 'gmail', 'cloudflare_email')
  ),
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

INSERT INTO email_provider_events_0193_new
SELECT * FROM email_provider_events;

DROP TABLE email_provider_events;
DROP TABLE email_drafts;
DROP TABLE email_deliveries;
DROP TABLE email_messages;

ALTER TABLE email_messages_0193_new RENAME TO email_messages;
ALTER TABLE email_deliveries_0193_new RENAME TO email_deliveries;
ALTER TABLE email_drafts_0193_new RENAME TO email_drafts;
ALTER TABLE email_provider_events_0193_new RENAME TO email_provider_events;

CREATE INDEX IF NOT EXISTS idx_email_messages_kind_status
  ON email_messages(kind, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_messages_target_user
  ON email_messages(target_user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_deliveries_message
  ON email_deliveries(message_id, attempted_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_deliveries_provider_status
  ON email_deliveries(provider, status, attempted_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_drafts_message
  ON email_drafts(message_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS email_provider_events_type_created_idx
  ON email_provider_events(provider, event_type, created_at DESC);

PRAGMA foreign_keys = ON;
