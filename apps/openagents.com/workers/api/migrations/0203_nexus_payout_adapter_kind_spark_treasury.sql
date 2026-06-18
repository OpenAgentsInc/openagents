-- openagents #5232: the real Tassadar run-settlement rail uses the
-- `spark_treasury` payout adapter, but the original nexus treasury payout
-- ledger tables (migration 0122) constrained `adapter_kind` to
-- ('hosted_mdk','legacy_nexus_import','mdk_agent_wallet','simulation').
--
-- Because the ledger writes use `INSERT OR IGNORE`, a real `spark_treasury`
-- intent/attempt/reconciliation row was SILENTLY DROPPED by the CHECK
-- constraint, so the gated real payout never persisted an intent and the
-- dispatch surfaced a generic `adapter_unavailable` with no `/spark/pay` call.
--
-- SQLite cannot ALTER an existing CHECK constraint, so this migration rebuilds
-- the three affected tables with an `adapter_kind` CHECK that matches the typed
-- `NexusTreasuryPayoutAdapterKind` schema (adds 'spark_treasury'). Data is
-- copied verbatim; column order, indexes, and foreign keys are preserved.
-- FK enforcement is disabled for the rebuild so the intermediate rename/create
-- ordering (where dependents briefly reference a not-yet-recreated parent) is
-- safe; all parent rows are copied back before the file completes, so the final
-- state satisfies every foreign key.

PRAGMA foreign_keys = OFF;

-- 1) nexus_treasury_payout_intents -------------------------------------------
ALTER TABLE nexus_treasury_payout_intents
  RENAME TO nexus_treasury_payout_intents_old;

CREATE TABLE nexus_treasury_payout_intents (
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
      'simulation',
      'spark_treasury'
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

INSERT INTO nexus_treasury_payout_intents (
  id, payout_intent_ref, idempotency_key_hash, actor_ref, owner_user_id,
  source_kind, buyer_payment_ref, accepted_work_refs_json, assignment_ref,
  artanis_dispatch_ref, pylon_job_ref, payout_target_ref,
  payout_target_approval_ref, adapter_kind, amount_asset, amount_denomination,
  amount_minor_units, spend_cap_asset, spend_cap_denomination,
  spend_cap_amount_minor_units, policy_snapshot_ref, status, metadata_refs_json,
  public_projection_json, created_at, updated_at, archived_at
)
SELECT
  id, payout_intent_ref, idempotency_key_hash, actor_ref, owner_user_id,
  source_kind, buyer_payment_ref, accepted_work_refs_json, assignment_ref,
  artanis_dispatch_ref, pylon_job_ref, payout_target_ref,
  payout_target_approval_ref, adapter_kind, amount_asset, amount_denomination,
  amount_minor_units, spend_cap_asset, spend_cap_denomination,
  spend_cap_amount_minor_units, policy_snapshot_ref, status, metadata_refs_json,
  public_projection_json, created_at, updated_at, archived_at
FROM nexus_treasury_payout_intents_old;

DROP TABLE nexus_treasury_payout_intents_old;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_intents_assignment_idx
  ON nexus_treasury_payout_intents(assignment_ref, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_intents_status_idx
  ON nexus_treasury_payout_intents(status, updated_at DESC)
  WHERE archived_at IS NULL;

-- 2) nexus_treasury_payout_attempts ------------------------------------------
ALTER TABLE nexus_treasury_payout_attempts
  RENAME TO nexus_treasury_payout_attempts_old;

CREATE TABLE nexus_treasury_payout_attempts (
  id TEXT PRIMARY KEY NOT NULL,
  payout_attempt_ref TEXT NOT NULL UNIQUE,
  payout_intent_ref TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  adapter_kind TEXT NOT NULL CHECK (
    adapter_kind IN (
      'hosted_mdk',
      'legacy_nexus_import',
      'mdk_agent_wallet',
      'simulation',
      'spark_treasury'
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

INSERT INTO nexus_treasury_payout_attempts (
  id, payout_attempt_ref, payout_intent_ref, idempotency_key_hash, adapter_kind,
  adapter_attempt_ref, status, redacted_payment_ref, redacted_destination_ref,
  amount_asset, amount_denomination, amount_minor_units, metadata_refs_json,
  public_projection_json, created_at, updated_at, archived_at
)
SELECT
  id, payout_attempt_ref, payout_intent_ref, idempotency_key_hash, adapter_kind,
  adapter_attempt_ref, status, redacted_payment_ref, redacted_destination_ref,
  amount_asset, amount_denomination, amount_minor_units, metadata_refs_json,
  public_projection_json, created_at, updated_at, archived_at
FROM nexus_treasury_payout_attempts_old;

DROP TABLE nexus_treasury_payout_attempts_old;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_attempts_intent_idx
  ON nexus_treasury_payout_attempts(payout_intent_ref, updated_at DESC)
  WHERE archived_at IS NULL;

-- 3) nexus_treasury_payout_reconciliation_events -----------------------------
ALTER TABLE nexus_treasury_payout_reconciliation_events
  RENAME TO nexus_treasury_payout_reconciliation_events_old;

CREATE TABLE nexus_treasury_payout_reconciliation_events (
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
      'simulation',
      'spark_treasury'
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

INSERT INTO nexus_treasury_payout_reconciliation_events (
  id, event_ref, idempotency_key_hash, provider_ref, external_event_ref,
  adapter_kind, payout_intent_ref, payout_attempt_ref, status, result_ref,
  metadata_refs_json, public_projection_json, created_at, archived_at
)
SELECT
  id, event_ref, idempotency_key_hash, provider_ref, external_event_ref,
  adapter_kind, payout_intent_ref, payout_attempt_ref, status, result_ref,
  metadata_refs_json, public_projection_json, created_at, archived_at
FROM nexus_treasury_payout_reconciliation_events_old;

DROP TABLE nexus_treasury_payout_reconciliation_events_old;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_reconciliation_created_idx
  ON nexus_treasury_payout_reconciliation_events(created_at DESC)
  WHERE archived_at IS NULL;
