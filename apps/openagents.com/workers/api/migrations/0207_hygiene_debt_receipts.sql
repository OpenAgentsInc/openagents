-- #5372 (EPIC #5335): durable store of CREATED, PAYABLE hygiene debt receipts.
--
-- This is the create-side of the funded hygiene-lane settlement flow (lane
-- settlement process step 1): the requester / settlement-authority turns a
-- merged + reviewed debt into a funded, PAYABLE receipt, keyed by its typed
-- `DebtReceiptKey` (#5340). The settle route resolves payability ONLY from this
-- store; there is no other source of truth for a payable debt receipt.
--
-- One row per DebtReceiptKey. A `payable` row is settleable exactly once; once
-- settled it transitions to `retired` and is no longer payable (a second settle
-- on the same key reprojects to `duplicate_replay`). Retired keys are not
-- re-creatable.
--
-- Every column carries PUBLIC-SAFE refs only (paths/counts/digests, no code
-- bodies, no raw diffs, no wallet/payment material, no addresses, no
-- timestamps inside refs). The canonical `settlement_input_json` is re-validated
-- by the debt-receipt policy on every reprojection, so the store never relies on
-- itself to keep secrets out.
CREATE TABLE IF NOT EXISTS hygiene_debt_receipts (
  -- The typed DebtReceiptKey (#5340): debt_receipt_key:<sha256>.
  debt_receipt_key TEXT PRIMARY KEY,
  -- 'payable' | 'retired'. A retired key is no longer payable.
  state TEXT NOT NULL DEFAULT 'payable',
  -- Denormalized DebtReceiptKey input components (query/audit only).
  debt_receipt_ref TEXT NOT NULL,
  repo_baseline_ref TEXT NOT NULL,
  scope_digest TEXT NOT NULL,
  objective_digest TEXT NOT NULL,
  -- The merged-PR + reviewer-acceptance refs this receipt funds.
  merged_pr_ref TEXT NOT NULL,
  reviewer_acceptance_ref TEXT NOT NULL,
  -- Baseline / target metric refs and the verifier-command refs (JSON arrays).
  baseline_metric_refs_json TEXT NOT NULL,
  target_metric_refs_json TEXT NOT NULL,
  verification_command_refs_json TEXT NOT NULL,
  -- The settlement-authority actor ref (the principal that funded the receipt).
  settlement_authority_actor_ref TEXT,
  -- Budget cap and the formula-independent payable amount (sats).
  budget_cap_sats INTEGER NOT NULL,
  payable_sats INTEGER NOT NULL,
  -- The canonical full debt-receipt policy input (JSON). Projection authority.
  settlement_input_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  -- Retirement bookkeeping: set when the key settles.
  retired_at TEXT,
  settlement_receipt_ref TEXT
);

CREATE INDEX IF NOT EXISTS idx_hygiene_debt_receipts_state
  ON hygiene_debt_receipts(state, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_hygiene_debt_receipts_merged_pr
  ON hygiene_debt_receipts(merged_pr_ref);
