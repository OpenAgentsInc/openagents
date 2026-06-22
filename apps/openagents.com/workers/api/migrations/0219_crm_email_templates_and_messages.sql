-- 0219_crm_email_templates_and_messages.sql
--
-- CRM email templates + per-contact send ledger (epic #5980, sub-issue #5983).
--
-- The Gmail/gws channel (and later Resend, #5984) renders a contact-personalized
-- message from a template and records the outcome against the contact. These
-- tables are the CRM-specific template library + send ledger (distinct from the
-- campaign-shaped global `email_messages` in 0026): every row ties to a
-- crm_contact, carries the send `channel`, and supports the local Gmail
-- executor's write-back (draft id / message id).
--
-- Tenant-scoped, ISO-8601 TEXT timestamps, enums via CHECK, soft-delete via
-- archived_at — matching 0218_crm_contacts.sql.

CREATE TABLE IF NOT EXISTS crm_email_templates (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  subject_template TEXT NOT NULL,
  body_markdown_template TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_email_templates_tenant_slug_idx
  ON crm_email_templates(tenant_ref, slug);

CREATE TABLE IF NOT EXISTS crm_email_messages (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  template_id TEXT REFERENCES crm_email_templates(id) ON DELETE SET NULL,
  channel TEXT NOT NULL CHECK (channel IN ('gmail_gws', 'resend')),
  from_email TEXT,
  to_email TEXT NOT NULL,
  subject TEXT NOT NULL,
  body_markdown TEXT NOT NULL,
  body_html TEXT,
  status TEXT NOT NULL DEFAULT 'draft'
    CHECK (status IN ('draft', 'queued', 'sent', 'failed')),
  send_reason TEXT,
  provider_message_id TEXT,
  provider_draft_id TEXT,
  error_message TEXT,
  sent_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_email_messages_contact_idx
  ON crm_email_messages(contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS crm_email_messages_tenant_idx
  ON crm_email_messages(tenant_ref, created_at DESC);
