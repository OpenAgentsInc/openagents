-- Business signup + opt-in Slack Connect request intake (Epic C / C4).
--
-- The public /business form can request a shared Slack channel, but Slack
-- Connect still requires explicit operator handling and the other workspace's
-- acceptance. This table stores the request and the manual invite state; it
-- does not grant Slack, workspace, spend, payout, or agent authority.

CREATE TABLE IF NOT EXISTS business_signup_requests (
  id TEXT PRIMARY KEY NOT NULL,
  business_name TEXT NOT NULL,
  contact_email TEXT NOT NULL,
  website TEXT,
  phone TEXT NOT NULL,
  help_with TEXT,
  request_slack_channel INTEGER NOT NULL DEFAULT 0 CHECK (
    request_slack_channel IN (0, 1)
  ),
  slack_connect_status TEXT NOT NULL CHECK (
    slack_connect_status IN (
      'not_requested',
      'manual_invite_pending',
      'invite_sent',
      'accepted',
      'declined'
    )
  ),
  source_route TEXT NOT NULL DEFAULT '/business',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS business_signup_requests_email_idx
  ON business_signup_requests(contact_email, created_at DESC);

CREATE INDEX IF NOT EXISTS business_signup_requests_slack_status_idx
  ON business_signup_requests(slack_connect_status, created_at DESC)
  WHERE request_slack_channel = 1;
