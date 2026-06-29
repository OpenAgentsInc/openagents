-- Forum work-request negotiation stores public quote refs and one winning
-- acceptance per request. Escrow reserve receipts are refs into the labor
-- escrow ledger; no invoice, preimage, wallet, or provider credential material
-- belongs in these tables.

CREATE TABLE IF NOT EXISTS forum_work_request_offers (
  id TEXT PRIMARY KEY NOT NULL,
  work_request_id TEXT NOT NULL REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  quote_ref TEXT NOT NULL UNIQUE,
  provider_actor_ref TEXT NOT NULL,
  amount_sats INTEGER NOT NULL CHECK (amount_sats > 0),
  amount_msats INTEGER NOT NULL CHECK (amount_msats > 0),
  capability_refs_json TEXT NOT NULL DEFAULT '[]',
  relay_event_ref TEXT,
  state TEXT NOT NULL DEFAULT 'offered' CHECK (
    state IN ('offered', 'accepted', 'rejected', 'expired')
  ),
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_offers_request
  ON forum_work_request_offers(work_request_id, state, created_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS forum_work_request_acceptances (
  id TEXT PRIMARY KEY NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  work_request_id TEXT NOT NULL UNIQUE REFERENCES forum_work_requests(id)
    ON DELETE CASCADE,
  offer_id TEXT NOT NULL REFERENCES forum_work_request_offers(id)
    ON DELETE CASCADE,
  quote_ref TEXT NOT NULL,
  requester_actor_ref TEXT NOT NULL,
  provider_actor_ref TEXT NOT NULL,
  amount_msats INTEGER NOT NULL CHECK (amount_msats > 0),
  escrow_id TEXT NOT NULL UNIQUE,
  reserve_receipt_ref TEXT NOT NULL UNIQUE,
  acceptance_event_ref TEXT NOT NULL,
  public_projection_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_forum_work_request_acceptances_quote
  ON forum_work_request_acceptances(quote_ref)
  WHERE archived_at IS NULL;
