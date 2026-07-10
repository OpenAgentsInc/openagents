-- FC-3 (#8639): accepted-Pylon steering delivery outcomes.
--
-- The request body remains in `khala_sync_fleet_steering_intents`, where the
-- owner submitted it. This table is deliberately body-free: the standing
-- Pylon acknowledges only the exact intent identity and a typed outcome.
-- Effective run/approval Sync post-images are appended in the same transaction
-- as an `applied` outcome; other states never masquerade as executor-effective.

ALTER TABLE sarah_fleet_run_intake_leases
  ADD CONSTRAINT sarah_fleet_run_intake_leases_exchange_identity_unique
  UNIQUE (run_ref, claim_ref, owner_user_id, pylon_ref);

ALTER TABLE khala_sync_fleet_steering_intents
  ADD CONSTRAINT khala_sync_fleet_steering_intents_delivery_identity_unique
  UNIQUE (run_ref, seq, intent_id),
  ADD CONSTRAINT khala_sync_fleet_steering_intents_intent_id_length
  CHECK (length(intent_id) BETWEEN 1 AND 160),
  ADD CONSTRAINT khala_sync_fleet_steering_intents_idempotency_key_length
  CHECK (length(idempotency_key) BETWEEN 8 AND 120);

CREATE TABLE IF NOT EXISTS sarah_fleet_run_steering_deliveries (
  run_ref          text   NOT NULL,
  seq              bigint NOT NULL CHECK (seq >= 1),
  intent_id        text   NOT NULL,
  owner_user_id    text   NOT NULL,
  pylon_ref        text   NOT NULL,
  intake_claim_ref text   NOT NULL,
  delivered_at     text   NOT NULL,
  PRIMARY KEY (run_ref, seq),
  UNIQUE (intent_id),
  CONSTRAINT sarah_fleet_run_steering_deliveries_identity_unique
    UNIQUE (
      run_ref,
      seq,
      intent_id,
      intake_claim_ref,
      owner_user_id,
      pylon_ref
    ),
  CONSTRAINT sarah_fleet_run_steering_deliveries_intent_fk
    FOREIGN KEY (run_ref, seq, intent_id)
      REFERENCES khala_sync_fleet_steering_intents(run_ref, seq, intent_id),
  CONSTRAINT sarah_fleet_run_steering_deliveries_claim_fk
    FOREIGN KEY (run_ref, intake_claim_ref, owner_user_id, pylon_ref)
      REFERENCES sarah_fleet_run_intake_leases(
        run_ref,
        claim_ref,
        owner_user_id,
        pylon_ref
      ),
  CONSTRAINT sarah_fleet_run_steering_deliveries_run_ref_shape
    CHECK (run_ref ~ '^fleet_run\.sarah\.[0-9a-f]{20}$'),
  CONSTRAINT sarah_fleet_run_steering_deliveries_intent_ref_shape
    CHECK (intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$'),
  CONSTRAINT sarah_fleet_run_steering_deliveries_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_steering_deliveries_pylon_shape
    CHECK (pylon_ref ~ '^[a-z0-9][a-z0-9._:-]{2,119}$'),
  CONSTRAINT sarah_fleet_run_steering_deliveries_claim_shape
    CHECK (intake_claim_ref ~ '^claim\.sarah_fleet_run\.[0-9a-f]{24}$')
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_steering_deliveries_claim_seq_idx
  ON sarah_fleet_run_steering_deliveries
    (intake_claim_ref, run_ref, seq);

CREATE TABLE IF NOT EXISTS sarah_fleet_run_steering_outcomes (
  run_ref          text   NOT NULL,
  seq              bigint NOT NULL CHECK (seq >= 1),
  intent_id        text   NOT NULL,
  owner_user_id    text   NOT NULL,
  pylon_ref        text   NOT NULL,
  intake_claim_ref text   NOT NULL,
  outcome           text   NOT NULL
    CHECK (outcome IN (
      'applied',
      'queued_follow_up',
      'skipped_stale',
      'rejected',
      'failed'
    )),
  outcome_ref       text   NOT NULL UNIQUE,
  observed_at       text   NOT NULL,
  recorded_at       text   NOT NULL,
  PRIMARY KEY (run_ref, seq),
  UNIQUE (intent_id),
  CONSTRAINT sarah_fleet_run_steering_outcomes_run_fk
    FOREIGN KEY (run_ref) REFERENCES sarah_fleet_run_requests(run_ref)
      ON DELETE CASCADE,
  CONSTRAINT sarah_fleet_run_steering_outcomes_intent_fk
    FOREIGN KEY (run_ref, seq, intent_id)
      REFERENCES khala_sync_fleet_steering_intents(run_ref, seq, intent_id),
  CONSTRAINT sarah_fleet_run_steering_outcomes_claim_fk
    FOREIGN KEY (run_ref, intake_claim_ref, owner_user_id, pylon_ref)
      REFERENCES sarah_fleet_run_intake_leases(
        run_ref,
        claim_ref,
        owner_user_id,
        pylon_ref
      ),
  CONSTRAINT sarah_fleet_run_steering_outcomes_delivery_fk
    FOREIGN KEY (
      run_ref,
      seq,
      intent_id,
      intake_claim_ref,
      owner_user_id,
      pylon_ref
    ) REFERENCES sarah_fleet_run_steering_deliveries(
      run_ref,
      seq,
      intent_id,
      intake_claim_ref,
      owner_user_id,
      pylon_ref
    ),
  CONSTRAINT sarah_fleet_run_steering_outcomes_run_ref_shape
    CHECK (run_ref ~ '^fleet_run\.sarah\.[0-9a-f]{20}$'),
  CONSTRAINT sarah_fleet_run_steering_outcomes_intent_ref_shape
    CHECK (intent_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,159}$'),
  CONSTRAINT sarah_fleet_run_steering_outcomes_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_fleet_run_steering_outcomes_pylon_shape
    CHECK (pylon_ref ~ '^[a-z0-9][a-z0-9._:-]{2,119}$'),
  CONSTRAINT sarah_fleet_run_steering_outcomes_claim_shape
    CHECK (intake_claim_ref ~ '^claim\.sarah_fleet_run\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_fleet_run_steering_outcomes_outcome_ref_shape
    CHECK (outcome_ref ~ '^outcome\.pylon\.fleet_steering\.[a-f0-9]{24}$')
);

CREATE INDEX IF NOT EXISTS sarah_fleet_run_steering_outcomes_claim_seq_idx
  ON sarah_fleet_run_steering_outcomes
    (intake_claim_ref, run_ref, seq);
