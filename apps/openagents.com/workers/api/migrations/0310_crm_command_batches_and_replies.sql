-- OB-4 (#8561): batch approval UX receipts + inbound reply routing plumbing.
--
-- The existing law (epic #5980) stays unchanged: outbound send is
-- approval-gated (`crm_contact_commands` propose -> operator approve ->
-- `dispatchCrmSend`) and every send keeps its own individual receipt on that
-- row's `result_json`. This migration adds a BATCH UX rollup receipt (NOT a
-- batch authority) so a day's queue can be reviewed/approved in one operator
-- action while every underlying send still gets executed and receipted one
-- at a time through the unchanged `approveAndExecuteCrmSendCommand` path.

-- crm_command_batches: one row per batch-approve OPERATOR ACTION. Records
-- which commands were included and how each disposed (executed/failed/
-- not_pending/not_found/capped by the daily send cap) — the batch-level
-- receipt that sits alongside the per-command receipts already on
-- crm_contact_commands.result_json.
CREATE TABLE IF NOT EXISTS crm_command_batches (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  approved_by_ref TEXT,
  command_ids_json TEXT NOT NULL DEFAULT '[]',
  requested_count INTEGER NOT NULL DEFAULT 0,
  executed_count INTEGER NOT NULL DEFAULT 0,
  failed_count INTEGER NOT NULL DEFAULT 0,
  not_pending_count INTEGER NOT NULL DEFAULT 0,
  not_found_count INTEGER NOT NULL DEFAULT 0,
  capped_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_command_batches_tenant_idx
  ON crm_command_batches(tenant_ref, created_at DESC);

-- crm_reply_events: inbound reply plumbing. The Sarah repo (OpenAgentsInc/
-- sarah) S-8 email channel is the intended long-term producer of these rows
-- once it ships; until then this table + the matching route accept the same
-- event shape from a manual/test/inbound-webhook source so the CRM-side
-- wiring (suppression + activity logging) is ready to receive real traffic.
-- `routed_to` records where the reply was handed off: 'sarah_inbox' once S-8
-- lands, 'operator_notification' for the v0 fallback named in the issue.
CREATE TABLE IF NOT EXISTS crm_reply_events (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT REFERENCES crm_contacts(id) ON DELETE SET NULL,
  from_email TEXT NOT NULL,
  subject TEXT,
  body_text TEXT,
  in_reply_to_ref TEXT,
  provider TEXT NOT NULL DEFAULT 'inbound_webhook',
  provider_event_id TEXT,
  opt_out INTEGER NOT NULL DEFAULT 0 CHECK (opt_out IN (0, 1)),
  routed_to TEXT NOT NULL DEFAULT 'operator_notification'
    CHECK (routed_to IN ('sarah_inbox', 'operator_notification')),
  created_at TEXT NOT NULL
);

-- Idempotent replay of the same provider event (both non-null).
CREATE UNIQUE INDEX IF NOT EXISTS crm_reply_events_provider_event_idx
  ON crm_reply_events(provider, provider_event_id)
  WHERE provider_event_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS crm_reply_events_contact_idx
  ON crm_reply_events(contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS crm_reply_events_tenant_idx
  ON crm_reply_events(tenant_ref, created_at DESC);
