CREATE INDEX IF NOT EXISTS buyer_payment_reconciliation_events_receipt_idx
  ON buyer_payment_reconciliation_events(receipt_ref, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS nexus_treasury_payout_intents_buyer_payment_idx
  ON nexus_treasury_payout_intents(buyer_payment_ref, updated_at DESC)
  WHERE archived_at IS NULL;
