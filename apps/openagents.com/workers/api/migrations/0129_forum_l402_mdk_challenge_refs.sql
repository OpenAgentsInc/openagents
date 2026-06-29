ALTER TABLE forum_l402_challenges
  ADD COLUMN recipient_actor_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN recipient_readiness_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_provider_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_environment TEXT CHECK (
    mdk_environment IS NULL OR mdk_environment IN ('production', 'sandbox')
  );

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_sandbox INTEGER CHECK (
    mdk_sandbox IS NULL OR mdk_sandbox IN (0, 1)
  );

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_implementation_state TEXT CHECK (
    mdk_implementation_state IS NULL OR mdk_implementation_state IN (
      'fake_provider_contract',
      'live_provider_configured',
      'missing_configuration'
    )
  );

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_checkout_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_checkout_url_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_checkout_launch_path TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_invoice_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN mdk_payment_hash_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN l402_credential_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN l402_replay_nonce_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN l402_endpoint_ref TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN l402_entitlement_scope_refs_json TEXT;

ALTER TABLE forum_l402_challenges
  ADD COLUMN l402_www_authenticate TEXT;

CREATE INDEX IF NOT EXISTS idx_forum_l402_challenges_recipient
  ON forum_l402_challenges(recipient_actor_ref, recipient_readiness_ref, created_at DESC)
  WHERE archived_at IS NULL;
