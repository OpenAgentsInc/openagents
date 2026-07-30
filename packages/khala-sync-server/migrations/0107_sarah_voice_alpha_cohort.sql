-- A Sarah voice alpha admission is explicit and revocable. Finite account
-- credit remains a separate payment authority.

CREATE TABLE IF NOT EXISTS sarah_voice_alpha_memberships (
  membership_ref        text PRIMARY KEY,
  cohort_ref            text NOT NULL,
  owner_user_id         text NOT NULL UNIQUE,
  state                 text NOT NULL CHECK (state IN ('active', 'revoked')),
  admitted_at           text NOT NULL,
  admission_actor_ref   text NOT NULL,
  admission_reason      text NOT NULL,
  revoked_at            text,
  revocation_actor_ref  text,
  revocation_reason     text,
  updated_at            text NOT NULL,
  CHECK (
    (state = 'active' AND revoked_at IS NULL
      AND revocation_actor_ref IS NULL AND revocation_reason IS NULL)
    OR
    (state = 'revoked' AND revoked_at IS NOT NULL
      AND revocation_actor_ref IS NOT NULL AND revocation_reason IS NOT NULL)
  )
);

CREATE INDEX IF NOT EXISTS sarah_voice_alpha_memberships_active_cohort_idx
  ON sarah_voice_alpha_memberships (cohort_ref, state);

CREATE TABLE IF NOT EXISTS sarah_voice_alpha_membership_audit (
  event_ref       text PRIMARY KEY,
  membership_ref text NOT NULL,
  cohort_ref     text NOT NULL,
  action          text NOT NULL CHECK (action IN ('admitted', 'revoked')),
  actor_ref       text NOT NULL,
  reason          text NOT NULL,
  source          text NOT NULL,
  occurred_at     text NOT NULL
);

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS admission_cohort_ref text;
