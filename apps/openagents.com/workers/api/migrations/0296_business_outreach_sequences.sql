-- LG-4 approval-gated outreach template and sequence tooling (#8265).
--
-- Stores only opaque subject refs, pipeline refs, audit/finding refs, template
-- refs, approval receipt refs, mailbox refs, and rendered public-safe draft
-- text. Raw prospect names, email addresses, domains, Apollo payloads, and CRM
-- notes do not belong here.

CREATE TABLE IF NOT EXISTS business_outreach_template_approvals (
  approval_receipt_ref TEXT PRIMARY KEY NOT NULL,
  template_version_ref TEXT NOT NULL,
  approved_by_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_outreach_template_approvals_template
  ON business_outreach_template_approvals(template_version_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS business_outreach_suppressions (
  suppression_ref TEXT PRIMARY KEY NOT NULL,
  subject_ref TEXT NOT NULL,
  reason TEXT NOT NULL CHECK (
    reason IN ('existing_partner', 'existing_customer', 'active_intake')
  ),
  source_ref TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_business_outreach_suppressions_subject_reason
  ON business_outreach_suppressions(subject_ref, reason);

CREATE TABLE IF NOT EXISTS business_outreach_drafts (
  draft_ref TEXT PRIMARY KEY NOT NULL,
  pipeline_ref TEXT NOT NULL REFERENCES business_pipeline_rows(pipeline_ref),
  subject_ref TEXT NOT NULL,
  template_version_ref TEXT NOT NULL,
  segment_ref TEXT NOT NULL,
  audit_report_ref TEXT NOT NULL,
  finding_refs_json TEXT NOT NULL DEFAULT '[]',
  body_text TEXT NOT NULL,
  claim_lint_refs_json TEXT NOT NULL DEFAULT '[]',
  source_ref TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'draft' CHECK (state IN ('draft')),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_outreach_drafts_pipeline
  ON business_outreach_drafts(pipeline_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_business_outreach_drafts_subject
  ON business_outreach_drafts(subject_ref, created_at DESC);

CREATE TABLE IF NOT EXISTS business_outreach_sends (
  send_ref TEXT PRIMARY KEY NOT NULL,
  pipeline_ref TEXT NOT NULL REFERENCES business_pipeline_rows(pipeline_ref),
  draft_ref TEXT NOT NULL REFERENCES business_outreach_drafts(draft_ref),
  subject_ref TEXT NOT NULL,
  template_version_ref TEXT NOT NULL,
  mailbox_ref TEXT NOT NULL,
  channel TEXT NOT NULL CHECK (
    channel IN ('apollo_sequence', 'customer_mailbox', 'manual')
  ),
  source_ref TEXT NOT NULL,
  approval_receipt_ref TEXT NOT NULL
    REFERENCES business_outreach_template_approvals(approval_receipt_ref),
  send_receipt_ref TEXT NOT NULL UNIQUE,
  sent_at TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_business_outreach_sends_mailbox_day
  ON business_outreach_sends(mailbox_ref, sent_at);

CREATE INDEX IF NOT EXISTS idx_business_outreach_sends_pipeline
  ON business_outreach_sends(pipeline_ref, created_at DESC);
