-- MPP Lightning charge consume-once cache (EPIC #6049, draft-lightning-charge-00).
--
-- The Lightning charge intent requires ATOMIC consume-once settlement
-- (§"Settlement Procedure" + §"Preimage Confidentiality"): a challenge marked
-- consumed MUST NOT be accepted again, and concurrent requests presenting the
-- same valid preimage MUST yield exactly one success. We claim the paymentHash
-- BEFORE serving; a replay collides on the PRIMARY KEY and is rejected before a
-- second free completion is served.
--
-- We key on the PAYMENT HASH (the public payment identifier), never the
-- preimage. The preimage is a bearer secret and MUST NOT be logged, persisted,
-- or returned (spec §"Preimage Confidentiality"); only its sha256 (the payment
-- hash) is recorded here. This mirrors the SPT replay cache (0224).
--
-- INERT NOTE: this table is only written on the Lightning rail, which itself is
-- inert until KHALA_MPP_LIGHTNING_ENABLED + KHALA_MPP_ENABLED + the MDK wallet
-- binding are all configured. No other rail touches it.

CREATE TABLE IF NOT EXISTS mpp_lightning_replay (
  -- The BOLT11 payment hash (sha256 of the preimage), lowercase hex. PRIMARY KEY
  -- so a second use of the same paid invoice collides and is refused.
  payment_hash TEXT PRIMARY KEY,
  -- The challenge id the payment was consumed under (binds the proof to the
  -- exact quote, for audit).
  challenge_id TEXT NOT NULL,
  consumed_at TEXT NOT NULL
);
