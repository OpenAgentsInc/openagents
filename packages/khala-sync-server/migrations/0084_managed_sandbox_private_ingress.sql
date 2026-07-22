-- SBX-10: digest-only private ingress capability lifecycle.
--
-- The URL is not stored. The row binds only its digest, owner, audience,
-- sandbox generation, short expiry, terminal cleanup, and command receipts.

ALTER TABLE khala_sync_managed_sandbox_phase2_operations
  DROP CONSTRAINT IF EXISTS khala_sync_managed_sandbox_phase2_operations_command_kind_check;

ALTER TABLE khala_sync_managed_sandbox_phase2_operations
  ADD CONSTRAINT khala_sync_managed_sandbox_phase2_operations_command_kind_check
  CHECK (command_kind IN (
    'CreateCheckpoint', 'ArchiveWithCheckpoint', 'ForkFromCheckpoint',
    'RestoreCheckpoint', 'DeleteCheckpoint', 'CreatePrivateIngress',
    'RevokePrivateIngress', 'ExpirePrivateIngress'
  ));

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_private_ingress (
  capability_ref text PRIMARY KEY,
  owner_user_id text NOT NULL,
  tenant_ref text NOT NULL,
  sandbox_ref text NOT NULL,
  resource_generation bigint NOT NULL CHECK (resource_generation >= 0),
  audience_ref text NOT NULL,
  access_url_digest text NOT NULL CHECK (access_url_digest ~ '^sha256:[a-f0-9]{64}$'),
  capability_state text NOT NULL CHECK (capability_state IN ('Active', 'Cleaned')),
  capability_fingerprint text NOT NULL CHECK (capability_fingerprint ~ '^sha256:[a-f0-9]{64}$'),
  capability_json jsonb NOT NULL CHECK (jsonb_typeof(capability_json) = 'object'),
  created_by_command_ref text NOT NULL
    REFERENCES khala_sync_managed_sandbox_phase2_operations(command_ref),
  updated_by_command_ref text NOT NULL
    REFERENCES khala_sync_managed_sandbox_phase2_operations(command_ref),
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (owner_user_id, tenant_ref, capability_ref)
);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_private_ingress_expiry
  ON khala_sync_managed_sandbox_private_ingress(capability_state, expires_at);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_private_ingress_sandbox
  ON khala_sync_managed_sandbox_private_ingress(
    owner_user_id, tenant_ref, sandbox_ref, resource_generation
  );
