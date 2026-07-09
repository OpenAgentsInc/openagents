-- 0311_crm_sarah_handoff_links.sql
--
-- OB-5 (#8562): reply-to-conversation handoff links.
--
-- The Sarah spec (docs/fable/2026-07-07-sarah-sales-agent-spec.md §5) names
-- an opaque `prospect_ref` minted on first meaningful interaction, carried
-- across web/email, and bound to a `crm_contacts` row (the CRM contact
-- becomes the join key across channels). The durable prospect session itself
-- lives in the separate private `OpenAgentsInc/sarah` repo (S-1/S-3), which is
-- not reachable from this repo. This table is the LINK ISSUANCE side owned
-- here: every time a reply is routed back to a prospect (OB-4's
-- `crm-reply.ts`), we mint an unguessable handoff token bound to the CRM
-- contact (+ optional opportunity + LG-6 source ref) and hand the prospect a
-- URL into sarah.openagents.com carrying that token. When the Sarah repo's
-- own durable-session machinery reads the token, it can bind or create its
-- `prospect_ref` session using it as the continuation key — that exchange
-- happens on the other side of the repo boundary.
--
-- Mirrors the OB-3 `agent_readiness_public_reports` shape (migration 0310):
-- an unguessable token primary lookup key, click tracking, and internal refs
-- (tenant/contact/opportunity/source) that are never returned to the token
-- holder. This table is intentionally NOT part of the khala-sync Postgres
-- mirror registry (crm-email-domain-store.ts) — same precedent as
-- `agent_readiness_public_reports`, which also stays D1-only.

CREATE TABLE IF NOT EXISTS crm_sarah_handoff_links (
  id TEXT PRIMARY KEY NOT NULL,
  handoff_token TEXT NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  opportunity_id TEXT REFERENCES crm_opportunities(id) ON DELETE SET NULL,
  source_ref TEXT NOT NULL DEFAULT 'unknown',
  reply_event_id TEXT,
  click_count INTEGER NOT NULL DEFAULT 0,
  last_clicked_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_sarah_handoff_links_token_idx
  ON crm_sarah_handoff_links(handoff_token);

CREATE INDEX IF NOT EXISTS crm_sarah_handoff_links_contact_idx
  ON crm_sarah_handoff_links(contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS crm_sarah_handoff_links_tenant_idx
  ON crm_sarah_handoff_links(tenant_ref, created_at DESC);
