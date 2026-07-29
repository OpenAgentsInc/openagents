-- #9272: A staging-only owner entitlement can waive the Sarah voice credit
-- hold and debit. Authentication, usage records, concurrency, and lifetime
-- limits continue to apply.

CREATE TABLE IF NOT EXISTS sarah_voice_credit_entitlements (
  entitlement_ref         text PRIMARY KEY,
  owner_user_id            text NOT NULL UNIQUE,
  environment              text NOT NULL CHECK (environment = 'staging'),
  state                    text NOT NULL CHECK (state IN ('active', 'revoked')),
  activated_at             text NOT NULL,
  expires_at               text NOT NULL,
  activation_reason        text NOT NULL,
  activation_actor_ref     text NOT NULL,
  activation_source        text NOT NULL,
  revoked_at               text,
  revocation_actor_ref     text,
  revocation_reason        text,
  updated_at               text NOT NULL,
  CHECK (
    (state = 'active' AND revoked_at IS NULL
      AND revocation_actor_ref IS NULL AND revocation_reason IS NULL)
    OR
    (state = 'revoked' AND revoked_at IS NOT NULL
      AND revocation_actor_ref IS NOT NULL AND revocation_reason IS NOT NULL)
  )
);

CREATE TABLE IF NOT EXISTS sarah_voice_credit_entitlement_audit (
  event_ref                text PRIMARY KEY,
  entitlement_ref          text NOT NULL,
  action                   text NOT NULL CHECK (action IN ('activated', 'revoked')),
  actor_ref                text NOT NULL,
  reason                   text NOT NULL,
  source                   text NOT NULL,
  occurred_at              text NOT NULL
);

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS credit_mode text NOT NULL DEFAULT 'metered';

ALTER TABLE sarah_realtime_voice_sessions
  ADD COLUMN IF NOT EXISTS entitlement_ref text;

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_reserved_msat_check;

ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_charged_msat_check;

-- PostgreSQL gives the original multi-column inline check this short name.
ALTER TABLE sarah_realtime_voice_sessions
  DROP CONSTRAINT IF EXISTS sarah_realtime_voice_sessions_check;

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_mode_check
  CHECK (credit_mode IN ('metered', 'staging_owner_entitlement'));

ALTER TABLE sarah_realtime_voice_sessions
  ADD CONSTRAINT sarah_realtime_voice_sessions_credit_accounting_check
  CHECK (
    (credit_mode = 'metered' AND reserved_msat > 0
      AND charged_msat >= 0 AND charged_msat <= reserved_msat
      AND entitlement_ref IS NULL)
    OR
    (credit_mode = 'staging_owner_entitlement' AND reserved_msat = 0
      AND charged_msat >= 0 AND entitlement_ref IS NOT NULL)
  );
