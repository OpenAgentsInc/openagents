-- #9285: Production-alpha Sarah voice is owner-waived and unmetered. Provider
-- usage remains evidence, but these sessions neither reserve nor debit the
-- OpenAgents credit ledger.

ALTER TABLE sarah_voice_admissions
  DROP CONSTRAINT IF EXISTS sarah_voice_admissions_credit_mode_check;

ALTER TABLE sarah_voice_admissions
  ADD CONSTRAINT sarah_voice_admissions_credit_mode_check CHECK (
    credit_mode IN (
      'metered',
      'staging_owner_entitlement',
      'owner_waived_unmetered'
    )
  );

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_credit_mode_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_mode_check CHECK (
    credit_mode IN (
      'metered',
      'staging_owner_entitlement',
      'owner_waived_unmetered'
    )
  );

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_credit_accounting_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_accounting_check CHECK (
    (credit_mode = 'metered' AND reserved_msat > 0
      AND charged_msat >= 0 AND charged_msat <= reserved_msat
      AND entitlement_ref IS NULL)
    OR
    (credit_mode = 'staging_owner_entitlement' AND reserved_msat = 0
      AND charged_msat >= 0 AND entitlement_ref IS NOT NULL)
    OR
    (credit_mode = 'owner_waived_unmetered' AND reserved_msat = 0
      AND charged_msat = 0 AND entitlement_ref IS NULL)
  );

CREATE TABLE IF NOT EXISTS sarah_voice_accounting_waivers (
  waiver_ref                         text PRIMARY KEY,
  waiver_receipt_ref                 text NOT NULL UNIQUE,
  waiver_payload_digest              text NOT NULL UNIQUE CHECK (
    waiver_payload_digest ~ '^[0-9a-f]{64}$'
  ),
  session_ref                        text NOT NULL UNIQUE
    REFERENCES sarah_realtime_voice_sessions (session_ref),
  generation                         integer NOT NULL CHECK (generation > 0),
  operator_actor_ref                 text NOT NULL,
  waiver_reason                      text NOT NULL,
  provider_evidence_refs_json        jsonb NOT NULL CHECK (
    jsonb_typeof(provider_evidence_refs_json) = 'array'
  ),
  provider_session_ref_digest        text NOT NULL CHECK (
    provider_session_ref_digest ~ '^[0-9a-f]{64}$'
  ),
  prior_reserved_msat                bigint NOT NULL CHECK (prior_reserved_msat >= 0),
  prior_recorded_charge_msat         bigint NOT NULL CHECK (
    prior_recorded_charge_msat >= 0
  ),
  provider_accounting_status         text NOT NULL CHECK (
    provider_accounting_status = 'uncertain'
  ),
  authority                          text NOT NULL CHECK (
    authority = 'owner_waived_unmetered_v1'
  ),
  created_at                         text NOT NULL
);
