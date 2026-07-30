CREATE TABLE IF NOT EXISTS sarah_voice_admissions (
  admission_ref                     text PRIMARY KEY,
  owner_user_id                     text NOT NULL,
  device_ref                        text NOT NULL,
  thread_ref                        text NOT NULL,
  session_ref                       text NOT NULL,
  generation                        bigint NOT NULL CHECK (generation >= 0),
  disclosure_ref                    text NOT NULL,
  client_profile                    text NOT NULL,
  admission_cohort_ref              text NOT NULL,
  credit_mode                       text NOT NULL CHECK (
    credit_mode IN ('metered', 'staging_owner_entitlement')
  ),
  terms_digest                      text NOT NULL CHECK (
    terms_digest ~ '^[0-9a-f]{64}$'
  ),
  spendable_remaining_credit_msat   bigint CHECK (
    spendable_remaining_credit_msat IS NULL
    OR spendable_remaining_credit_msat >= 0
  ),
  state                             text NOT NULL CHECK (
    state IN ('active', 'consumed', 'expired')
  ),
  issued_at                         text NOT NULL,
  expires_at                        text NOT NULL,
  consumed_at                       text,
  CHECK (
    (state IN ('active', 'expired') AND consumed_at IS NULL)
    OR (state = 'consumed' AND consumed_at IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS sarah_voice_admissions_owner_state_expiry_idx
  ON sarah_voice_admissions (owner_user_id, state, expires_at);
