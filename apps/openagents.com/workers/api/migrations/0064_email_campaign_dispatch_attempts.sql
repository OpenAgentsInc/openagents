ALTER TABLE email_campaign_sends
  ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0);

ALTER TABLE email_campaign_sends
  ADD COLUMN next_attempt_at TEXT;

CREATE INDEX IF NOT EXISTS email_campaign_sends_dispatch_due_idx
  ON email_campaign_sends(status, due_at ASC, next_attempt_at ASC);
