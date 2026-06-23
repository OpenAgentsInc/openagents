-- MPP Stripe SPT single-use replay cache (EPIC #6049, defect B).
--
-- The Payment Auth core spec requires single-use payment proofs, and the Stripe
-- charge intent (draft-stripe-charge-00 §"Verification Procedure" step 4) makes
-- the server enforce that an SPT (`spt_…`) is not used twice. Stripe also
-- prevents SPT reuse at the API level, and the PaymentIntent idempotency key
-- (`<challengeId>_<spt>`) prevents duplicate charges; this local cache is the
-- defense-in-depth replay guard the spec says servers MAY/SHOULD additionally
-- keep. We record the consumed SPT keyed on its id; a second attempt collides on
-- the PRIMARY KEY and is rejected before any second Stripe charge.
--
-- INERT NOTE: this table is only written on the card/SPT rail, which itself is
-- inert until STRIPE_MPP_NETWORK_PROFILE_ID + KHALA_MPP_ENABLED + STRIPE_API_KEY
-- are all configured. The crypto rail does not touch it.

CREATE TABLE IF NOT EXISTS mpp_spt_replay (
  -- The Shared Payment Token id (starts with `spt_`).
  spt TEXT PRIMARY KEY,
  -- The challenge id the SPT was consumed under (binds the proof to the quote).
  challenge_id TEXT NOT NULL,
  -- The resulting Stripe PaymentIntent id, for dereference.
  payment_intent_id TEXT,
  consumed_at TEXT NOT NULL
);
