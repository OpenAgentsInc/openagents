ALTER TABLE business_service_promises
  ADD COLUMN blocking_reason_ref TEXT;

ALTER TABLE business_service_promises
  ADD COLUMN blocked_at TEXT;

ALTER TABLE business_service_promises
  ADD COLUMN last_escalation_page_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_business_service_promises_blocked
  ON business_service_promises(state, blocked_at, updated_at ASC);

CREATE TABLE IF NOT EXISTS business_fulfillment_escalation_pages (
  id TEXT PRIMARY KEY NOT NULL,
  promise_id TEXT NOT NULL
    REFERENCES business_service_promises(id) ON DELETE CASCADE,
  promise_ref TEXT NOT NULL,
  escalation_date TEXT NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  page_ref TEXT NOT NULL UNIQUE,
  owner_notification_ref TEXT NOT NULL,
  agent_definition_ref TEXT NOT NULL,
  blocking_reason_ref TEXT NOT NULL,
  blocked_at TEXT NOT NULL,
  workspace_ref TEXT NOT NULL,
  stakeholder_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  UNIQUE (promise_id, escalation_date)
);

CREATE INDEX IF NOT EXISTS idx_business_fulfillment_escalation_pages_promise
  ON business_fulfillment_escalation_pages(promise_id, created_at DESC);
