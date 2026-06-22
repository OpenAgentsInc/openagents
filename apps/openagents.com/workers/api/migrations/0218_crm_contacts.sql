-- 0218_crm_contacts.sql
--
-- Native, tenant-scoped contact CRM model (epic #5980, sub-issue #5981).
--
-- The Cloudflare Worker already has campaign / subscriber / prospect tables
-- (0026, 0063, 0181, 0072 ...), but NO relationship-CRM model: contacts,
-- accounts, lists, activities, engagement, opportunities. This migration adds
-- that model so the same engine serves OpenAgents' own outreach AND every
-- customer's, with strict per-tenant isolation.
--
-- Conventions (match recent migrations, e.g. 0214_partner_agreements.sql):
--   * TEXT primary keys (compactRandomId refs), ISO-8601 TEXT timestamps.
--   * tenant_ref on every row is the owner/team scope (multi-tenant isolation).
--   * enums via CHECK constraints; booleans as INTEGER 0/1; JSON as *_json TEXT.
--   * soft-delete via archived_at (NULL = active); partial indexes filter it.
--
-- This migration is data-model only. No behavior changes; read APIs and the
-- writers that use these tables ship in the same epic.

-- Accounts (organizations / funds) -----------------------------------------
CREATE TABLE IF NOT EXISTS crm_accounts (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  name TEXT NOT NULL,
  domain TEXT,
  account_type TEXT NOT NULL DEFAULT 'company',
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'archived')),
  website_url TEXT,
  notes TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_accounts_tenant_name_idx
  ON crm_accounts(tenant_ref, name)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS crm_accounts_tenant_domain_idx
  ON crm_accounts(tenant_ref, domain)
  WHERE archived_at IS NULL;

-- Contacts ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS crm_contacts (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  primary_email TEXT NOT NULL,
  secondary_email TEXT,
  full_name TEXT,
  first_name TEXT,
  last_name TEXT,
  job_title TEXT,
  contact_type TEXT NOT NULL DEFAULT 'prospect',
  relationship_stage TEXT NOT NULL DEFAULT 'new',
  lifecycle_stage TEXT NOT NULL DEFAULT 'lead',
  account_id TEXT REFERENCES crm_accounts(id) ON DELETE SET NULL,
  portal_access_status TEXT NOT NULL DEFAULT 'none',
  engagement_score INTEGER NOT NULL DEFAULT 0,
  last_contacted_at TEXT,
  last_engaged_at TEXT,
  last_replied_at TEXT,
  external_source_label TEXT,
  external_source_id TEXT,
  notes TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

-- Per-tenant dedupe key: one contact per (tenant, lowercased email).
CREATE UNIQUE INDEX IF NOT EXISTS crm_contacts_tenant_email_idx
  ON crm_contacts(tenant_ref, primary_email);

CREATE INDEX IF NOT EXISTS crm_contacts_tenant_listing_idx
  ON crm_contacts(tenant_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS crm_contacts_account_idx
  ON crm_contacts(account_id)
  WHERE archived_at IS NULL;

-- Contact lists (segments) --------------------------------------------------
CREATE TABLE IF NOT EXISTS crm_contact_lists (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT,
  is_system INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_contact_lists_tenant_slug_idx
  ON crm_contact_lists(tenant_ref, slug);

CREATE TABLE IF NOT EXISTS crm_contact_list_memberships (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  list_id TEXT NOT NULL REFERENCES crm_contact_lists(id) ON DELETE CASCADE,
  membership_status TEXT NOT NULL DEFAULT 'active'
    CHECK (membership_status IN ('active', 'removed')),
  source TEXT NOT NULL DEFAULT 'manual',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_contact_list_memberships_pair_idx
  ON crm_contact_list_memberships(contact_id, list_id);

-- Activities (audit log) ----------------------------------------------------
CREATE TABLE IF NOT EXISTS crm_activities (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  account_id TEXT REFERENCES crm_accounts(id) ON DELETE SET NULL,
  activity_type TEXT NOT NULL,
  subject TEXT,
  summary TEXT,
  occurred_at TEXT NOT NULL,
  actor_ref TEXT,
  source_system TEXT NOT NULL DEFAULT 'crm',
  source_record_type TEXT,
  source_record_id TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_activities_contact_idx
  ON crm_activities(contact_id, occurred_at DESC);

-- Dedupe key for backfilled / provider-sourced activities (both non-null).
CREATE UNIQUE INDEX IF NOT EXISTS crm_activities_source_idx
  ON crm_activities(source_record_type, source_record_id)
  WHERE source_record_type IS NOT NULL AND source_record_id IS NOT NULL;

-- Engagement snapshots (cached rollups) -------------------------------------
CREATE TABLE IF NOT EXISTS crm_engagement_snapshots (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  last_email_sent_at TEXT,
  last_email_opened_at TEXT,
  last_email_clicked_at TEXT,
  last_email_replied_at TEXT,
  email_sent_count_30d INTEGER NOT NULL DEFAULT 0,
  email_open_count_30d INTEGER NOT NULL DEFAULT 0,
  email_click_count_30d INTEGER NOT NULL DEFAULT 0,
  engagement_score INTEGER NOT NULL DEFAULT 0,
  snapshot_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_engagement_snapshots_contact_idx
  ON crm_engagement_snapshots(contact_id);

-- Opportunities (deal pipeline) ---------------------------------------------
CREATE TABLE IF NOT EXISTS crm_opportunities (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  account_id TEXT REFERENCES crm_accounts(id) ON DELETE SET NULL,
  name TEXT NOT NULL,
  round_name TEXT,
  stage TEXT NOT NULL DEFAULT 'sourcing',
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'won', 'lost')),
  target_amount_cents INTEGER,
  expected_amount_cents INTEGER,
  conviction_probability INTEGER,
  target_close_date TEXT,
  summary TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS crm_opportunities_tenant_idx
  ON crm_opportunities(tenant_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS crm_opportunity_contact_roles (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  opportunity_id TEXT NOT NULL REFERENCES crm_opportunities(id) ON DELETE CASCADE,
  contact_id TEXT NOT NULL REFERENCES crm_contacts(id) ON DELETE CASCADE,
  role_type TEXT NOT NULL DEFAULT 'participant',
  status TEXT NOT NULL DEFAULT 'active',
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS crm_opportunity_contact_roles_pair_idx
  ON crm_opportunity_contact_roles(opportunity_id, contact_id);

-- Approval-gated contact mutations (maps onto the Blueprint action model) ----
CREATE TABLE IF NOT EXISTS crm_contact_commands (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  contact_id TEXT REFERENCES crm_contacts(id) ON DELETE SET NULL,
  command_kind TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'proposed'
    CHECK (status IN ('proposed', 'approved', 'applied', 'rejected', 'failed')),
  proposed_by_ref TEXT,
  approval_state TEXT NOT NULL DEFAULT 'pending_approval',
  payload_json TEXT NOT NULL DEFAULT '{}',
  result_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_contact_commands_tenant_idx
  ON crm_contact_commands(tenant_ref, created_at DESC);

-- Import audit (one row per bulk import run) --------------------------------
CREATE TABLE IF NOT EXISTS crm_source_import_runs (
  id TEXT PRIMARY KEY NOT NULL,
  tenant_ref TEXT NOT NULL,
  source_label TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'completed'
    CHECK (status IN ('running', 'completed', 'failed')),
  total_rows INTEGER NOT NULL DEFAULT 0,
  imported_rows INTEGER NOT NULL DEFAULT 0,
  updated_rows INTEGER NOT NULL DEFAULT 0,
  duplicate_rows INTEGER NOT NULL DEFAULT 0,
  failed_rows INTEGER NOT NULL DEFAULT 0,
  error_summary TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS crm_source_import_runs_tenant_idx
  ON crm_source_import_runs(tenant_ref, created_at DESC);
