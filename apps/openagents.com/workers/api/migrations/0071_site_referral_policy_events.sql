CREATE TABLE IF NOT EXISTS site_referral_policy_events (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  subject_kind TEXT NOT NULL CHECK (
    subject_kind IN (
      'referral_source',
      'referral_invite',
      'referral_attribution',
      'user_attribution',
      'order_attribution',
      'agent_attribution',
      'workflow_event'
    )
  ),
  subject_ref TEXT NOT NULL,
  referral_attribution_id TEXT
    REFERENCES referral_attributions(id) ON DELETE SET NULL,
  referral_source_id TEXT
    REFERENCES site_referral_sources(id) ON DELETE SET NULL,
  referral_invite_id TEXT REFERENCES referral_invites(id) ON DELETE SET NULL,
  referral_workflow_event_id TEXT
    REFERENCES referral_workflow_events(id) ON DELETE SET NULL,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  previous_state TEXT,
  decision_state TEXT NOT NULL CHECK (
    decision_state IN (
      'pending',
      'active',
      'held',
      'disputed',
      'capped',
      'reversed',
      'expired',
      'archived',
      'operator_overridden'
    )
  ),
  policy_reason TEXT NOT NULL CHECK (
    policy_reason IN (
      'eligible',
      'self_referral',
      'duplicate_account',
      'collusion_risk',
      'chargeback_refund',
      'sanctions_compliance',
      'expired',
      'cap_exceeded',
      'clawback',
      'operator_override',
      'refund_or_reversal',
      'first_verified_wins',
      'manual_review'
    )
  ),
  eligibility TEXT NOT NULL CHECK (
    eligibility IN ('eligible', 'not_eligible', 'manual_review')
  ),
  customer_status TEXT NOT NULL CHECK (
    customer_status IN ('active', 'under_review', 'not_eligible', 'expired')
  ),
  operator_actor_user_id TEXT,
  operator_note_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  decided_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_site_referral_policy_events_source
  ON site_referral_policy_events(referral_source_id, decided_at DESC)
  WHERE referral_source_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_site_referral_policy_events_attribution
  ON site_referral_policy_events(referral_attribution_id, decided_at DESC)
  WHERE referral_attribution_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_site_referral_policy_events_workflow
  ON site_referral_policy_events(referral_workflow_event_id, decided_at DESC)
  WHERE referral_workflow_event_id IS NOT NULL AND archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_site_referral_policy_events_subject
  ON site_referral_policy_events(subject_kind, subject_ref, decided_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_site_referral_policy_events_decision
  ON site_referral_policy_events(decision_state, decided_at DESC)
  WHERE archived_at IS NULL;
