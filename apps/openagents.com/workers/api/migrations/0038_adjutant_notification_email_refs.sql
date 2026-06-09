ALTER TABLE adjutant_assignment_events
  ADD COLUMN email_message_id TEXT REFERENCES email_messages(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS adjutant_assignment_events_email_message_idx
  ON adjutant_assignment_events(email_message_id)
  WHERE email_message_id IS NOT NULL;

ALTER TABLE site_events
  ADD COLUMN email_message_id TEXT REFERENCES email_messages(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS site_events_email_message_idx
  ON site_events(email_message_id)
  WHERE email_message_id IS NOT NULL;
