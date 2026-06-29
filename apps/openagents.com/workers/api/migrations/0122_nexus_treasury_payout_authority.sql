CREATE TABLE IF NOT EXISTS nexus_payout_target_approvals (
  id TEXT PRIMARY KEY NOT NULL,
  approval_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  payout_target_ref TEXT NOT NULL,
  redacted_destination_ref TEXT NOT NULL,
  owner_user_id TEXT,
  agent_ref TEXT,
  pylon_ref TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('active', 'expired', 'rejected', 'revoked')
  ),
  approved_by_ref TEXT NOT NULL,
  approval_policy_ref TEXT NOT NULL,
  scope_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  expires_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS nexus_payout_target_approvals_target_status_idx
  ON nexus_payout_target_approvals(payout_target_ref, status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_intents (
  id TEXT PRIMARY KEY NOT NULL,
  payout_intent_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  owner_user_id TEXT,
  source_kind TEXT NOT NULL CHECK (
    source_kind IN (
      'accepted_work',
      'forum_reward',
      'operator_test',
      'pylon_marketplace_assignment'
    )
  ),
  buyer_payment_ref TEXT,
  accepted_work_refs_json TEXT NOT NULL DEFAULT '[]',
  assignment_ref TEXT,
  artanis_dispatch_ref TEXT,
  pylon_job_ref TEXT,
  payout_target_ref TEXT NOT NULL,
  payout_target_approval_ref TEXT NOT NULL,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation'
    )
  ),
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL CHECK (
    amount_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  spend_cap_asset TEXT NOT NULL CHECK (spend_cap_asset IN ('bitcoin', 'credits', 'usd')),
  spend_cap_denomination TEXT NOT NULL CHECK (
    spend_cap_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  spend_cap_amount_minor_units INTEGER NOT NULL CHECK (
    spend_cap_amount_minor_units >= 0
  ),
  policy_snapshot_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'approved',
      'cancelled',
      'dispatched',
      'failed',
      'proposed',
      'rejected',
      'settled'
    )
  ),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (payout_target_approval_ref)
    REFERENCES nexus_payout_target_approvals(approval_ref)
);

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_intents_assignment_idx
  ON nexus_treasury_payout_intents(assignment_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_intents_status_idx
  ON nexus_treasury_payout_intents(status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  payout_attempt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation'
    )
  ),
  adapter_attempt_ref TEXT NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN (
      'confirmed',
      'dispatched',
      'failed',
      'pending',
      'rejected',
      'replayed'
    )
  ),
  redacted_payment_ref TEXT,
  redacted_destination_ref TEXT NOT NULL,
  amount_asset TEXT NOT NULL CHECK (amount_asset IN ('bitcoin', 'credits', 'usd')),
  amount_denomination TEXT NOT NULL CHECK (
    amount_denomination IN ('bitcoin_millisatoshi', 'credit', 'usd_cent')
  ),
  amount_minor_units INTEGER NOT NULL CHECK (amount_minor_units >= 0),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (payout_intent_ref)
    REFERENCES nexus_treasury_payout_intents(payout_intent_ref)
);

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_attempts_intent_idx
  ON nexus_treasury_payout_attempts(payout_intent_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_treasury_payout_reconciliation_events (
  id TEXT PRIMARY KEY NOT NULL,
  event_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  provider_ref TEXT NOT NULL,
  external_event_ref TEXT NOT NULL,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation'
    )
  ),
  payout_intent_ref TEXT,
  payout_attempt_ref TEXT,
  status TEXT NOT NULL CHECK (
    status IN ('matched', 'observed', 'rejected', 'replayed')
  ),
  result_ref TEXT NOT NULL,
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  UNIQUE (provider_ref, external_event_ref),
  FOREIGN KEY (payout_intent_ref)
    REFERENCES nexus_treasury_payout_intents(payout_intent_ref),
  FOREIGN KEY (payout_attempt_ref)
    REFERENCES nexus_treasury_payout_attempts(payout_attempt_ref)
);

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_reconciliation_created_idx
  ON nexus_treasury_payout_reconciliation_events(created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_payment_authority_receipts (
  id TEXT PRIMARY KEY NOT NULL,
  receipt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  payout_attempt_ref TEXT,
  event_ref TEXT,
  receipt_kind TEXT NOT NULL CHECK (
    receipt_kind IN (
      'attempt_recorded',
      'confirmation_recorded',
      'dispatch_recorded',
      'intent_created',
      'pause_recorded',
      'policy_rejected',
      'settlement_recorded',
      'verification_recorded'
    )
  ),
  audience TEXT NOT NULL CHECK (
    audience IN ('agent', 'customer', 'operator', 'public')
  ),
  metadata_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (payout_intent_ref)
    REFERENCES nexus_treasury_payout_intents(payout_intent_ref),
  FOREIGN KEY (payout_attempt_ref)
    REFERENCES nexus_treasury_payout_attempts(payout_attempt_ref),
  FOREIGN KEY (event_ref)
    REFERENCES nexus_treasury_payout_reconciliation_events(event_ref)
);

CREATE INDEX IF NOT EXISTS nexus_payment_authority_receipts_intent_idx
  ON nexus_payment_authority_receipts(payout_intent_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS nexus_release_gates (
  id TEXT PRIMARY KEY NOT NULL,
  gate_ref TEXT NOT NULL UNIQUE,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  gate_kind TEXT NOT NULL CHECK (
    gate_kind IN (
      'artanis_real_assignment',
      'artanis_simulated_assignment',
      'mdk_adapter',
      'operator_dashboard',
      'public_receipt',
      'pylon_api',
      'pylon_v02_release',
      'simulation_adapter'
    )
  ),
  status TEXT NOT NULL CHECK (
    status IN ('blocked', 'failed', 'passed', 'pending')
  ),
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS nexus_release_gates_kind_status_idx
  ON nexus_release_gates(gate_kind, status, updated_at DESC)
  WHERE archived_at IS NULL;
