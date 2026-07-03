ALTER TABLE omni_accepted_outcome_contracts
  ADD COLUMN committed_deliverables_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE omni_accepted_outcome_contracts
  ADD COLUMN service_promise_state TEXT NOT NULL DEFAULT 'not_promised'
  CHECK (
    service_promise_state IN (
      'not_promised',
      'proposed',
      'active',
      'fulfilled',
      'paused',
      'breached',
      'cancelled'
    )
  );

ALTER TABLE omni_accepted_outcome_contracts
  ADD COLUMN sla_terms_json TEXT NOT NULL DEFAULT '[]';

ALTER TABLE omni_accepted_outcome_contracts
  ADD COLUMN fulfillment_receipts_json TEXT NOT NULL DEFAULT '[]';
