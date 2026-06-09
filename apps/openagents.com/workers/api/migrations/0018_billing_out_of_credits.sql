CREATE TABLE IF NOT EXISTS billing_credit_notifications (
  user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('out_of_credits')),
  email TEXT,
  display_name TEXT NOT NULL,
  balance_cents INTEGER NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'sent', 'failed')),
  resend_email_id TEXT,
  error_message TEXT,
  idempotency_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (user_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_billing_credit_notifications_status
  ON billing_credit_notifications(status, updated_at DESC);
