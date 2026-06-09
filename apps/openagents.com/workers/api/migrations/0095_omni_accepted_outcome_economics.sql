CREATE TABLE IF NOT EXISTS omni_accepted_outcome_economics (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL REFERENCES omni_workrooms(id) ON DELETE CASCADE,
  accepted_outcome_contract_id TEXT
    REFERENCES omni_accepted_outcome_contracts(id) ON DELETE SET NULL,
  work_kind TEXT NOT NULL CHECK (
    work_kind IN (
      'site',
      'coding',
      'adjustment',
      'existing_project_import',
      'business',
      'legal_sensitive'
    )
  ),
  funding_mode TEXT NOT NULL CHECK (
    funding_mode IN (
      'free_beta',
      'credit_funded',
      'sats_funded',
      'internal_only'
    )
  ),
  buyer_price_asset TEXT NOT NULL CHECK (
    buyer_price_asset IN ('none', 'usd', 'credits', 'sats')
  ),
  buyer_price_cents INTEGER NOT NULL DEFAULT 0 CHECK (buyer_price_cents >= 0),
  credits_charged INTEGER NOT NULL DEFAULT 0 CHECK (credits_charged >= 0),
  sats_charged INTEGER NOT NULL DEFAULT 0 CHECK (sats_charged >= 0),
  runner_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (runner_cost_cents >= 0),
  provider_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (provider_cost_cents >= 0),
  retry_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (retry_cost_cents >= 0),
  review_minutes INTEGER NOT NULL DEFAULT 0 CHECK (review_minutes >= 0),
  review_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (review_cost_cents >= 0),
  artifact_cost_cents INTEGER NOT NULL DEFAULT 0 CHECK (artifact_cost_cents >= 0),
  total_cost_cents INTEGER NOT NULL DEFAULT 0,
  accepted_value_cents INTEGER NOT NULL DEFAULT 0 CHECK (accepted_value_cents >= 0),
  gross_margin_cents INTEGER NOT NULL DEFAULT 0,
  public_caveat_ref TEXT NOT NULL,
  internal_caveat_ref TEXT,
  no_settlement_implication INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_omni_outcome_economics_workroom_updated
  ON omni_accepted_outcome_economics(workroom_id, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_omni_outcome_economics_kind_mode
  ON omni_accepted_outcome_economics(work_kind, funding_mode, updated_at DESC)
  WHERE archived_at IS NULL;
