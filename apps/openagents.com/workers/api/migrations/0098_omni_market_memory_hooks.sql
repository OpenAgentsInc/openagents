CREATE TABLE IF NOT EXISTS omni_market_memory_hooks (
  id TEXT PRIMARY KEY,
  idempotency_key TEXT NOT NULL UNIQUE,
  workroom_id TEXT NOT NULL,
  lifecycle_decision_id TEXT NOT NULL,
  work_kind TEXT NOT NULL,
  outcome_state TEXT NOT NULL,
  category TEXT NOT NULL,
  memory_ref TEXT NOT NULL,
  evidence_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  public_caveat_ref TEXT NOT NULL,
  route_scorecard_ref TEXT,
  economics_ref TEXT,
  authority_boundary TEXT NOT NULL DEFAULT 'evidence_only',
  no_routing_mutation INTEGER NOT NULL DEFAULT 1,
  no_payout_mutation INTEGER NOT NULL DEFAULT 1,
  no_public_claim_mutation INTEGER NOT NULL DEFAULT 1,
  no_module_promotion INTEGER NOT NULL DEFAULT 1,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT,
  FOREIGN KEY (workroom_id) REFERENCES omni_workrooms(id),
  FOREIGN KEY (lifecycle_decision_id) REFERENCES omni_workroom_lifecycle_decisions(id)
);

CREATE INDEX IF NOT EXISTS idx_omni_market_memory_hooks_workroom_id
  ON omni_market_memory_hooks(workroom_id);

CREATE INDEX IF NOT EXISTS idx_omni_market_memory_hooks_category
  ON omni_market_memory_hooks(category, outcome_state, archived_at);
